/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use crate::{auth::Role, b64, error::Error::*, passwd::Password, Result};
use bson::oid::ObjectId;
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use futures::stream::{StreamExt, TryStreamExt};
use log;
use mongodb::bson::doc;
use mongodb::options::{ClientOptions, FindOneOptions, FindOptions, UpdateOptions};
use mongodb::results::UpdateResult;
use mongodb::{Client, Collection, Database};
use rand::{distributions::Distribution, Rng};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
use std::fmt;
use warp::Filter;
use webauthn_rs::proto::{Authentication, AuthenticatorData, Credential, CredentialID};
use webauthn_rs::{AuthenticationState, RegistrationState};

pub type PinType = u32;

#[derive(Deserialize, Serialize, Debug)]
pub struct UploadedFileVariant {
    #[serde(rename = "originalName")]
    pub original_name: String,
    #[serde(rename = "uploadedName")]
    pub uploaded_name: String,
    pub scale: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UploadedFile {
    #[serde(rename = "originalName")]
    pub original_name: String,
    #[serde(rename = "uploadedName")]
    pub uploaded_name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub retina: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub scale: Option<u32>,
    pub variants: Option<Vec<UploadedFileVariant>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Riddle {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(default)]
    pub difficulty: i32,
    #[serde(default)]
    pub deduction: Option<i32>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub files: Option<Vec<UploadedFile>>,
    pub ignore_case: Option<bool>,
    #[serde(default)]
    pub solution: String,
    #[serde(default)]
    pub debriefing: Option<String>,
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub credits: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub external_password_input: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Direction {
    pub direction: String,
    pub riddle_id: ObjectId,
    pub level: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Game {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Room {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub coords: Option<String>,
    pub neighbors: Vec<Direction>,
    #[serde(default)]
    pub game_id: ObjectId,
    #[serde(default)]
    pub entry: Option<bool>,
    #[serde(default)]
    pub exit: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SecondFactor {
    #[serde(rename = "TOTP")]
    Totp,
    #[serde(rename = "FIDO2")]
    Fido2,
}

impl SecondFactor {
    pub fn from_str(factor: &str) -> SecondFactor {
        match factor {
            "TOTP" => SecondFactor::Totp,
            "FIDO2" => SecondFactor::Fido2,
            _ => SecondFactor::Totp,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            SecondFactor::Totp => "TOTP",
            SecondFactor::Fido2 => "FIDO2",
        }
    }
}

impl fmt::Display for SecondFactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecondFactor::Totp => write!(f, "{}", SecondFactor::Totp.as_str()),
            SecondFactor::Fido2 => write!(f, "{}", SecondFactor::Fido2.as_str()),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct WebauthnManagementData {
    #[serde(default, rename = "registrationState")]
    pub registration_state: Option<RegistrationState>,
    #[serde(default)]
    pub credentials: Vec<Credential>,
    #[serde(default, rename = "authenticationState")]
    pub authentication_state: Option<AuthenticationState>,
}

impl WebauthnManagementData {
    pub fn new() -> WebauthnManagementData {
        WebauthnManagementData {
            registration_state: Option::default(),
            credentials: Vec::new(),
            authentication_state: Option::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct RiddleAttempt {
    pub riddle_id: ObjectId,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub t0: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub t_solved: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub username: String,
    pub email: String,
    pub role: Role,
    pub hash: String,
    #[serde(default)]
    pub pin: PinType,
    pub activated: bool,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub registered: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub last_login: Option<DateTime<Utc>>,
    pub solved: Vec<RiddleAttempt>,
    pub current_riddle_attempt: Option<RiddleAttempt>,
    #[serde(default)]
    pub rooms_entered: Vec<ObjectId>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub score: i32,
    pub in_room: Option<ObjectId>,
    #[serde(default)]
    pub awaiting_second_factor: bool,
    #[serde(default)]
    #[serde(with = "b64")]
    pub totp_key: Vec<u8>,
    #[serde(default)]
    pub recovery_keys: Vec<String>,
    #[serde(default)]
    pub webauthn: WebauthnManagementData,
}

#[derive(Deserialize, Debug)]
pub struct UserScoreData {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub username: String,
    #[serde(default)]
    pub solved: Vec<RiddleAttempt>,
    #[serde(default)]
    pub current_riddle_attempt: Option<RiddleAttempt>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub in_room: Option<ObjectId>,
}

impl User {
    pub fn new(
        username: &String,
        email: &String,
        role: Role,
        hash: String,
        pin: PinType,
        totp_key: Vec<u8>,
    ) -> Self {
        User {
            id: ObjectId::new(),
            username: username.to_string(),
            email: email.to_string(),
            role: role,
            hash: hash,
            pin: pin,
            activated: false,
            created: Some(Utc::now()),
            registered: Option::default(),
            last_login: Option::default(),
            solved: Vec::new(),
            current_riddle_attempt: Option::default(),
            rooms_entered: Vec::new(),
            level: 0,
            score: 0,
            in_room: Option::default(),
            awaiting_second_factor: false,
            totp_key: totp_key,
            recovery_keys: Vec::new(),
            webauthn: WebauthnManagementData::new(),
        }
    }
}

pub struct KeyChars;

impl Distribution<u8> for KeyChars {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> u8 {
        const RANGE: usize = 25 + 10;
        const GEN_ASCII_STR_CHARSET: &[u8; RANGE] = b"abcdefghijkmnopqrstuvwxyz0123456789";
        loop {
            let idx: usize = rng.next_u32() as usize % RANGE;
            return GEN_ASCII_STR_CHARSET[idx];
        }
    }
}

#[derive(Clone, Debug)]
pub struct DB {
    pub client: Client,
    pub name: String,
    pub coll_rooms: String,
    pub coll_riddles: String,
    pub coll_users: String,
}

impl DB {
    pub async fn init() -> Result<Self> {
        let url: String = env::var("DB_URL").expect("DB_URL is not in .env file");
        let name: String = env::var("DB_NAME").expect("DB_NAME is not in .env file");
        let coll_users: String =
            env::var("DB_COLL_USERS").expect("DB_COLL_USERS is not in .env file");
        let coll_riddles: String =
            env::var("DB_COLL_RIDDLES").expect("DB_COLL_RIDDLES is not in .env file");
        let coll_rooms: String =
            env::var("DB_COLL_ROOMS").expect("DB_COLL_ROOMS is not in .env file");
        let mut client_options: mongodb::options::ClientOptions =
            ClientOptions::parse(url).await.unwrap();
        client_options.app_name = Some(name.to_string());
        Ok(Self {
            client: Client::with_options(client_options).unwrap(),
            name: name.to_string(),
            coll_users: coll_users.to_string(),
            coll_riddles: coll_riddles.to_string(),
            coll_rooms: coll_rooms.to_string(),
        })
    }

    pub fn get_database(&self) -> Database {
        self.client.database(&self.name)
    }

    pub fn get_users_coll(&self) -> Collection<User> {
        self.get_database().collection::<User>(&self.coll_users)
    }

    pub fn get_riddles_coll(&self) -> Collection<Riddle> {
        self.get_database().collection::<Riddle>(&self.coll_riddles)
    }

    pub fn get_rooms_coll(&self) -> Collection<Room> {
        self.get_database().collection::<Room>(&self.coll_rooms)
    }

    pub async fn get_num_rooms(&self, game_id: &ObjectId) -> Result<u32> {
        log::info!("get_num_rooms(); game_id = {}", game_id);
        match self
            .get_rooms_coll()
            .count_documents(doc! { "game_id": game_id }, None)
            .await
        {
            Ok(count) => Ok(count as u32),
            Err(_) => return Err(RoomNotFoundError),
        }
    }

    pub async fn get_all_user_scores(&self) -> Result<Vec<UserScoreData>> {
        log::info!("get_all_user_scores()");
        let cursor: mongodb::Cursor<UserScoreData> = match self
            .get_database()
            .collection::<UserScoreData>(&self.coll_users)
            .find(
                doc! { "activated": true },
                FindOptions::builder()
                    .projection(doc! {
                        "username": 1u32,
                        "solved": 1u32,
                        "current_riddle_attempt": 1u32,
                        "level": 1u32,
                        "score": 1u32,
                        "in_room": 1u32,
                    })
                    .sort(doc! {
                        "score": 1u32,
                    })
                    .build(),
            )
            .await
        {
            Ok(cursor) => cursor,
            Err(e) => return Err(MongoQueryError(e)),
        };
        let users = match cursor.try_collect().await {
            Ok(users) => users,
            Err(e) => return Err(MongoError(e)),
        };
        Ok(users)
    }

    pub async fn get_max_score_for_game(&self, game_id: &ObjectId) -> Result<u32> {
        log::info!("get_max_score(); game_id = {}", game_id);
        let mut cursor: mongodb::Cursor<bson::Document> = match self
            .get_rooms_coll()
            .aggregate(
                vec![
                    doc! {
                        "$match": {
                            "game_id": game_id,
                        }
                    },
                    doc! {
                        "$unwind": "$neighbors",
                    },
                    doc! {
                        "$group": {
                            "_id": "$neighbors.riddle_id",
                        }
                    },
                    doc! {
                       "$lookup": {
                            "from": "riddles",
                            "localField": "_id",
                            "foreignField": "_id",
                            "as": "riddle"
                        }
                    },
                    doc! {
                        "$project": {
                            "score": doc! { "$arrayElemAt": [ "$riddle.difficulty", 0u32 ] }
                        }
                    },
                    doc! {
                        "$group": {
                            "_id": bson::Bson::Null,
                            "total": doc! { "$sum": "$score" }
                        }
                    },
                ],
                None,
            )
            .await
        {
            Ok(cursor) => cursor,
            Err(e) => return Err(MongoError(e)),
        };
        let result = match cursor.next().await {
            Some(result) => result,
            None => return Ok(0),
        };
        let doc: bson::Document = match result {
            Ok(doc) => doc,
            Err(e) => return Err(MongoError(e)),
        };
        let total = match doc.get("total") {
            Some(total) => total.as_i32().unwrap_or(0) as u32,
            None => 0,
        };
        Ok(total)
    }

    pub async fn get_num_riddles(&self, game_id: &ObjectId) -> Result<u32> {
        log::info!("get_num_riddles(); game_id = {}", game_id);
        let mut cursor: mongodb::Cursor<bson::Document> = match self
            .get_rooms_coll()
            .aggregate(
                vec![
                    doc! {
                        "$match": {
                            "game_id": game_id,
                        }
                    },
                    doc! {
                        "$unwind": "$neighbors",
                    },
                    doc! {
                        "$group": {
                            "_id": "$neighbors.riddle_id",
                        }
                    },
                    doc! {
                        "$group": {
                            "_id": bson::Bson::Null,
                            "count": { "$sum": 1u32 }
                        }
                    },
                ],
                None,
            )
            .await
        {
            Ok(cursor) => cursor,
            Err(e) => return Err(MongoError(e)),
        };
        let result = match cursor.next().await {
            Some(result) => result,
            None => return Ok(0),
        };
        let doc: bson::Document = match result {
            Ok(doc) => doc,
            Err(e) => return Err(MongoError(e)),
        };
        let count = match doc.get("count") {
            Some(count) => count.as_i32().unwrap_or(0) as u32,
            None => 0,
        };
        Ok(count)
    }

    pub async fn get_riddle_by_level(&self, level: u32) -> Result<Option<Riddle>> {
        log::info!("get_riddle_by_level(); level = {}", level);
        let riddle: Option<Riddle> = match self
            .get_riddles_coll()
            .find_one(doc! { "level": level }, None)
            .await
        {
            Ok(riddle) => riddle,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match riddle {
            Some(riddle) => Ok(Some(riddle)),
            None => {
                log::info!("riddle level {} not found", level);
                Ok(Option::default())
            }
        }
    }

    pub async fn get_riddle_by_oid(&self, oid: &ObjectId) -> Result<Option<Riddle>> {
        log::info!("get_riddle_by_oid(); oid = {}", oid);
        let riddle: Option<Riddle> = match self
            .get_riddles_coll()
            .find_one(doc! { "_id": oid }, None)
            .await
        {
            Ok(riddle) => riddle,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match riddle {
            Some(riddle) => {
                log::info!("Found riddle {} with level {}", oid, riddle.level);
                Ok(Some(riddle))
            }
            None => {
                log::info!("riddle not found");
                Ok(Option::default())
            }
        }
    }

    pub async fn get_riddle_if_solved(
        &self,
        riddle_id: &ObjectId,
        username: &String,
        options: impl Into<Option<FindOneOptions>>,
    ) -> Result<Option<Riddle>> {
        let user: Option<User> = match self
            .get_users_coll()
            .find_one(
                doc! {
                    "username": username,
                    "solved.riddle_id": riddle_id,
                },
                options,
            )
            .await
        {
            Ok(user) => user,
            Err(e) => return Err(MongoQueryError(e)),
        };
        if user.is_none() {
            return Ok(Option::default());
        }
        let riddle: Option<Riddle> = match self.get_riddle_by_oid(riddle_id).await {
            Ok(riddle) => riddle,
            Err(e) => return Err(e),
        };
        Ok(riddle)
    }

    pub async fn riddle_accessibility(
        &self,
        oid: &ObjectId,
        username: &String,
    ) -> (Option<ObjectId>, Option<User>, Option<String>) {
        // get the user associated with the request
        let user: User = match self.get_user(&username).await {
            Ok(user) => user,
            Err(e) => {
                return (Option::default(), Option::default(), Some(e.to_string()));
            }
        };
        // get the ID of the room the user is in
        let in_room: bson::oid::ObjectId = match user.in_room {
            Some(in_room) => in_room,
            None => {
                return (
                    Option::default(),
                    Option::default(),
                    Some("User is nowhere. That should not have happened :-/".to_string()),
                );
            }
        };
        // get the room
        let room: Room = match self.get_room(&in_room).await {
            Ok(room) => room,
            Err(e) => {
                return (Option::default(), Option::default(), Some(e.to_string()));
            }
        };
        // Check if one of the doorways is associated with the requested riddle.
        // This is to make sure, the user is not granted access to a riddle
        // they can't see from the current location (room).
        let found: &Direction = match room
            .neighbors
            .iter()
            .find(|neighbor| neighbor.riddle_id == *oid)
        {
            Some(neighbor) => neighbor,
            None => {
                return (
                    Option::default(),
                    Option::default(),
                    Some("doorway not accessible".to_string()),
                );
            }
        };
        (Some(found.riddle_id), Some(user), Option::default())
    }

    pub async fn is_username_or_email_taken(
        &self,
        username: &String,
        email: &String,
    ) -> Result<bool> {
        log::info!(
            "username_or_email_taken(); username = {}, email = {}",
            username,
            email
        );
        #[derive(Debug, Serialize, Deserialize)]
        struct UserId {
            _id: ObjectId,
        }
        let user: Option<UserId> = match self
            .get_database()
            .collection::<UserId>(&self.coll_users)
            .find_one(
                doc! {
                    "$or": vec![
                        doc! { "username": username },
                        doc! { "email": email }
                    ]
                },
                FindOneOptions::builder().build(),
            )
            .await
        {
            Ok(user) => user,
            Err(e) => {
                log::error!("{:?}", &e);
                return Err(MongoQueryError(e));
            }
        };
        match user {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    pub async fn get_user_role(&self, username: &String) -> Result<Role> {
        log::info!("get_user_role(); username = {}", username);
        #[derive(Debug, Serialize, Deserialize)]
        struct UserRole {
            _id: ObjectId,
            role: Role,
        }
        let user: Option<UserRole> = match self
            .get_database()
            .collection::<UserRole>(&self.coll_users)
            .find_one(
                doc! { "username": username },
                FindOneOptions::builder()
                    .projection(doc! { "role": 1u32 })
                    .build(),
            )
            .await
        {
            Ok(user) => user,
            Err(e) => {
                log::error!("{:?}", &e);
                return Err(MongoQueryError(e));
            }
        };
        match user {
            Some(user) => Ok(user.role),
            None => Err(UserNotFoundError),
        }
    }

    pub async fn get_user(&self, username: &String) -> Result<User> {
        log::info!("get_user(); username = {}", username);
        let user: Option<User> = match self
            .get_users_coll()
            .find_one(doc! { "username": username }, None)
            .await
        {
            Ok(user) => user,
            Err(e) => {
                log::error!("{:?}", &e);
                return Err(MongoQueryError(e));
            }
        };
        match user {
            Some(user) => Ok(user),
            None => Err(UserNotFoundError),
        }
    }

    pub async fn get_room(&self, oid: &ObjectId) -> Result<Room> {
        log::info!("get_room(); oid = {}", oid);
        let room: Option<Room> = match self
            .get_rooms_coll()
            .find_one(doc! { "_id": oid }, None)
            .await
        {
            Ok(room) => room,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match room {
            Some(room) => Ok(room),
            None => Err(RoomNotFoundError),
        }
    }

    pub async fn get_room_behind(
        &self,
        opposite: &String,
        riddle_id: &bson::oid::ObjectId,
    ) -> Result<Room> {
        log::info!(
            "get_room_behind(); opposite = {}, riddle_id = {}",
            opposite,
            riddle_id
        );
        let room: Option<Room> = match self
            .get_rooms_coll()
            .find_one(
                doc! {
                    "neighbors": {
                        "$elemMatch": {
                            "direction": opposite,
                            "riddle_id": riddle_id,
                        }
                    }
                },
                None,
            )
            .await
        {
            Ok(room) => room,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match room {
            Some(room) => Ok(room),
            None => Err(RoomBehindNotFoundError),
        }
    }

    pub async fn get_user_with_pin(&self, username: &String, pin: PinType) -> Result<User> {
        log::info!("get_user_with_pin(\"{}\", \"{:06}\")", username, pin);
        let result: Option<User> = match self
            .get_users_coll()
            .find_one(
                doc! { "username": username, "pin": pin, "activated": false },
                None,
            )
            .await
        {
            Ok(user) => user,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match result {
            Some(user) => {
                log::info!("Found {} <{}>", &user.username, &user.email);
                Ok(user)
            }
            None => {
                log::info!("user not found");
                Err(UserNotFoundError)
            }
        }
    }

    pub async fn set_user_solved(
        &mut self,
        solutions: &Vec<RiddleAttempt>,
        user: &User,
    ) -> Result<()> {
        match self
            .get_users_coll()
            .update_one(
                doc! { "_id": user.id, "activated": true },
                doc! {
                    "$set": {
                        "solved": bson::to_bson(solutions).unwrap(),
                        "level": user.level,
                        "score": user.score,
                    },
                },
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn update_webauthn_cred(
        &self,
        username: &String,
        cred_id: &CredentialID,
        auth_data: &AuthenticatorData<Authentication>,
    ) -> Result<()> {
        let update_options = UpdateOptions::builder()
            .array_filters(vec![doc! {
                "elem.cred_id": bson::to_bson(cred_id).unwrap(),
            }])
            .build();
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": username, "activated": true },
                doc! {
                    "$set": {
                        "webauthn.credentials.$[elem].counter": auth_data.counter,
                        "webauthn.credentials.$[elem].verified": auth_data.user_verified,
                    }
                },
                update_options,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn set_user_awaiting_2fa(&mut self, user: &User, awaiting: bool) -> Result<()> {
        match self
            .get_users_coll()
            .update_one(
                doc! { "_id": user.id, "activated": true },
                doc! {
                    "$set": { "awaiting_second_factor": awaiting },
                },
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn save_webauthn_registration_state(
        &self,
        username: &String,
        rs: &RegistrationState,
    ) -> Result<()> {
        log::info!(
            "save_webauthn_registration_state(); username = {}, rs = {:?}",
            username,
            rs
        );
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": username, "activated": true },
                doc! {
                    "$set": {
                        "webauthn.registrationState": Some(bson::to_bson(rs).unwrap()),
                    },
                },
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn save_webauthn_registration(
        &self,
        username: &String,
        creds: &Vec<Credential>,
    ) -> Result<()> {
        log::info!("save_webauthn_registration(); username = {}", username);
        dbg!(&creds);
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": username, "activated": true },
                doc! {
                    "$set": {
                        "webauthn.credentials": Some(bson::to_bson(creds).unwrap()),
                    },
                },
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn save_webauthn_authentication_state(
        &self,
        username: &String,
        st: &AuthenticationState,
    ) -> Result<()> {
        log::info!(
            "save_webauthn_authentication_state(); username = {}, as = {:?}",
            username,
            st
        );
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": username, "activated": true },
                doc! {
                    "$set": {
                        "webauthn.authenticationState": Some(bson::to_bson(st).unwrap()),
                    },
                },
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn rewrite_user_score(&mut self, user: &User) -> Result<()> {
        match self
            .get_users_coll()
            .update_one(
                doc! { "_id": user.id, "activated": true },
                doc! {
                    "$set": { "score": user.score },
                },
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn promote_user(&mut self, username: &String, role: &Role) -> Result<()> {
        let result: UpdateResult = match self
            .get_users_coll()
            .update_one(
                doc! { "username": username, "activated": true },
                doc! {
                    "$set": { "role": bson::to_bson(role).unwrap() },
                },
                None,
            )
            .await
        {
            Ok(result) => result,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match result {
            result if result.matched_count == 0 => Err(UserNotFoundError),
            result if result.matched_count == 1 && result.modified_count == 0 => {
                Err(CannotPromoteUserError)
            }
            _ => Ok(()),
        }
    }

    pub async fn create_user(&mut self, user: &User) -> Result<()> {
        log::info!("create_user({:?})", user);
        match self.get_users_coll().insert_one(user, None).await {
            Ok(_) => Ok(()),
            Err(e) => Err(MongoQueryError(e)),
        }
    }

    pub async fn login_user(&mut self, user: &User) -> Result<()> {
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": user.username.clone(), "activated": true },
                doc! {
                    "$set": {
                        "last_login": Some(Utc::now().timestamp()),
                        "awaiting_second_factor": false
                    },
                },
                None,
            )
            .await
        {
            Ok(_) => {
                log::info!("Updated {}.", &user.username);
                Ok(())
            }
            Err(e) => {
                log::error!("Error: update failed ({:?})", &e);
                Err(MongoQueryError(e))
            }
        }
    }

    pub async fn set_user_password(&mut self, username: &String, password: &String) -> Result<()> {
        let hash = match Password::hash(password) {
            Ok(hash) => hash,
            Err(e) => return Err(e),
        };
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": username, "activated": true },
                doc! {
                    "$set": {
                        "hash": hash,
                    },
                },
                None,
            )
            .await
        {
            Ok(_) => {
                log::info!("Updated {}.", username);
                Ok(())
            }
            Err(e) => {
                log::error!("Error: update failed ({:?})", &e);
                Err(MongoQueryError(e))
            }
        }
    }

    pub async fn activate_user(&mut self, user: &mut User) -> Result<()> {
        let entrance: Option<Room> = match self
            .get_rooms_coll()
            .find_one(
                doc! {
                    "entry": true,
                    /* XXX: choose a game_id */
                },
                None,
            )
            .await
        {
            Ok(entrance) => entrance,
            Err(e) => return Err(MongoQueryError(e)),
        };
        let first_room_id: bson::oid::ObjectId = match entrance {
            Some(room) => {
                log::info!("Found room {}", &room.id);
                room.id
            }
            None => return Err(RoomNotFoundError),
        };
        user.activated = true;
        user.registered = Some(Utc::now());
        user.last_login = Some(Utc::now());
        user.in_room = Some(first_room_id);
        user.rooms_entered.push(first_room_id);
        user.pin = 0;
        user.recovery_keys = (0..10)
            .map(|_| {
                let a: String = rand::thread_rng()
                    .sample_iter(&KeyChars)
                    .take(4)
                    .map(char::from)
                    .collect();
                let b: String = rand::thread_rng()
                    .sample_iter(&KeyChars)
                    .take(4)
                    .map(char::from)
                    .collect();
                let c: String = rand::thread_rng()
                    .sample_iter(&KeyChars)
                    .take(4)
                    .map(char::from)
                    .collect();
                let d: String = rand::thread_rng()
                    .sample_iter(&KeyChars)
                    .take(4)
                    .map(char::from)
                    .collect();
                a + "-" + &b + "-" + &c + "-" + &d
            })
            .collect();
        let modification: bson::Document = doc! {
            "$set": {
                "activated": user.activated,
                "registered": Utc::now().timestamp() as u32,
                "last_login": Utc::now().timestamp() as u32,
                "in_room": first_room_id,
                "rooms_entered": &user.rooms_entered,
                "recovery_keys": &user.recovery_keys,
            },
            "$unset": {
                "pin": 0 as u32,
            },
        };
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": user.username.clone(), "activated": false },
                modification,
                None,
            )
            .await
        {
            Ok(_) => {
                log::info!("Updated {}.", &user.username);
            }
            Err(e) => {
                log::error!("Error: update failed ({:?})", &e);
                return Err(MongoQueryError(e));
            }
        }
        Ok(())
    }
}

pub fn with_db(db: DB) -> impl Filter<Extract = (DB,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}
