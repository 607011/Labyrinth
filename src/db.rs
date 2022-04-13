/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use crate::{auth::Role, error::Error::*, Result};
use bson::oid::ObjectId;
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use mongodb::bson::doc;
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection, Database};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
use warp::Filter;

pub type PinType = u32;

#[derive(Deserialize, Serialize, Debug)]
pub struct UploadedFileVariant {
    #[serde(rename = "fileId")]
    pub file_id: ObjectId,
    pub name: String,
    pub scale: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UploadedFile {
    #[serde(rename = "fileId")]
    pub file_id: ObjectId,
    pub name: String,
    pub retina: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
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
    pub difficulty: u32,
    #[serde(default)]
    pub deduction: Option<u32>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub files: Option<Vec<UploadedFile>>,
    #[serde(default)]
    pub solution: String,
    #[serde(default)]
    pub debriefing: Option<String>,
    #[serde(default)]
    pub task: Option<String>,
    #[serde(default)]
    pub credits: Option<String>,
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

#[derive(Deserialize, Serialize, Debug)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub username: String,
    pub email: String,
    pub role: Role,
    pub hash: String,
    pub pin: Option<PinType>,
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
    pub solved: Vec<ObjectId>,
    #[serde(default)]
    pub rooms_entered: Vec<ObjectId>,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub score: u32,
    pub in_room: Option<ObjectId>,
}

impl User {
    pub fn new(
        username: &String,
        email: &String,
        role: Role,
        hash: String,
        pin: Option<PinType>,
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
            rooms_entered: Vec::new(),
            level: 0,
            score: 0,
            in_room: Option::default(),
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
        let url = env::var("DB_URL").expect("DB_URL is not in .env file");
        let name = env::var("DB_NAME").expect("DB_NAME is not in .env file");
        let coll_users = env::var("DB_COLL_USERS").expect("DB_COLL_USERS is not in .env file");
        let coll_riddles =
            env::var("DB_COLL_RIDDLES").expect("DB_COLL_RIDDLES is not in .env file");
        let coll_rooms = env::var("DB_COLL_ROOMS").expect("DB_COLL_ROOMS is not in .env file");
        let mut client_options = ClientOptions::parse(url).await.unwrap();
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

    pub async fn get_num_rooms(&self, game_id: &ObjectId) -> Result<Option<u32>> {
        println!("get_num_rooms(\"{}\")", game_id);
        match self
            .get_rooms_coll()
            .count_documents(doc! { "game_id": game_id }, None)
            .await
        {
            Ok(count) => Ok(Some(count as u32)),
            Err(_) => Ok(Option::default()),
        }
    }

    pub async fn get_num_riddles(&self, game_id: &ObjectId) -> Result<Option<u32>> {
        println!("get_num_riddles(\"{}\")", game_id);
        // XXX: wrong query
        match self
            .get_rooms_coll()
            .distinct("neighbors.riddle_id", doc! { "game_id": game_id }, None)
            .await
        {
            Ok(result) => Ok(Some(result.len() as u32)),
            Err(_) => Ok(Option::default()),
        }
    }

    pub async fn get_riddle_by_level(&self, level: u32) -> Result<Option<Riddle>> {
        println!("get_riddle_by_level(\"{}\")", level);
        let riddle = match self
            .get_riddles_coll()
            .find_one(doc! { "level": level }, None)
            .await
        {
            Ok(riddle) => riddle,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match riddle {
            Some(riddle) => {
                println!("Found {}", riddle.level);
                Ok(Some(riddle))
            }
            None => {
                println!("riddle not found");
                Ok(Option::default())
            }
        }
    }

    pub async fn get_riddle_by_oid(&self, oid: &ObjectId) -> Result<Option<Riddle>> {
        println!("get_riddle_by_oid(\"{:?}\")", oid);
        let riddle = match self
            .get_riddles_coll()
            .find_one(doc! { "_id": oid }, None)
            .await
        {
            Ok(riddle) => riddle,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match riddle {
            Some(riddle) => {
                println!("Found {}", riddle.level);
                Ok(Some(riddle))
            }
            None => {
                println!("riddle not found");
                Ok(Option::default())
            }
        }
    }

    pub async fn get_riddle_if_solved(
        &self,
        riddle_id: &ObjectId,
        username: &String,
    ) -> Result<Option<Riddle>> {
        let user = match self
            .get_users_coll()
            .find_one(
                doc! {
                    "username": username,
                    "solved": riddle_id,
                },
                None,
            )
            .await
        {
            Ok(user) => user,
            Err(e) => return Err(MongoQueryError(e)),
        };
        if user.is_none() {
            return Ok(Option::default());
        }
        let riddle = match self.get_riddle_by_oid(riddle_id).await {
            Ok(riddle) => riddle,
            Err(e) => return Err(e),
        };
        Ok(riddle)
    }

    pub async fn is_riddle_accessible(
        &self,
        oid: &ObjectId,
        username: &String,
    ) -> (Option<ObjectId>, Option<User>, Option<String>) {
        // get the user associated with the request
        let user = match self.get_user(&username).await {
            Ok(user) => user,
            Err(e) => {
                return (Option::default(), Option::default(), Some(e.to_string()));
            }
        };
        // get the ID of the room the user is in
        let in_room = match user.in_room {
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
        let room = match self.get_room(&in_room).await {
            Ok(room) => room,
            Err(e) => {
                return (Option::default(), Option::default(), Some(e.to_string()));
            }
        };
        // Check if one of the doorways is associated with the requested riddle.
        // This is to make sure, the user is not granted access to a riddle
        // they can't see from the current location (room).
        let found = match room
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

    pub async fn get_user(&self, username: &String) -> Result<User> {
        println!("get_user(\"{}\")", username);
        let user = match self
            .get_users_coll()
            .find_one(doc! { "username": username }, None)
            .await
        {
            Ok(user) => user,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match user {
            Some(user) => Ok(user),
            None => Err(UserNotFoundError),
        }
    }

    pub async fn get_room(&self, oid: &ObjectId) -> Result<Room> {
        println!("get_room(\"{}\")", oid);
        let room = match self
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
        println!("get_room_behind(\"{}\", \"{}\")", opposite, riddle_id);
        let result = self
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
            .await;
        let room = match result {
            Ok(room) => room,
            Err(e) => return Err(MongoQueryError(e)),
        };
        match room {
            Some(room) => Ok(room),
            None => Err(RoomBehindNotFoundError),
        }
    }

    pub async fn get_user_with_pin(&self, username: &String, pin: PinType) -> Result<User> {
        println!("get_user_with_pin(\"{}\", \"{:06}\")", username, pin);
        let result = match self
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
                println!("Found {} <{}>", user.username, user.email);
                Ok(user)
            }
            None => {
                println!("user not found");
                Err(UserNotFoundError)
            }
        }
    }

    pub async fn set_user_solved(
        &mut self,
        solutions: &Vec<bson::oid::ObjectId>,
        user: &User,
    ) -> Result<()> {
        match self
            .get_users_coll()
            .update_one(
                doc! { "_id": user.id, "activated": true },
                doc! {
                    "$set": { "solved": solutions, "level": user.level, "score": user.score },
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

    pub async fn create_user(&mut self, user: &User) -> Result<()> {
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
                    "$set": { "last_login": Some(Utc::now().timestamp()) },
                },
                None,
            )
            .await
        {
            Ok(_) => {
                println!("Updated {}.", user.username);
                Ok(())
            }
            Err(e) => {
                println!("Error: update failed ({:?})", e);
                Err(MongoQueryError(e))
            }
        }
    }

    pub async fn activate_user(&mut self, user: &mut User) -> Result<()> {
        let entrance = match self
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
        let first_room_id = match entrance {
            Some(room) => {
                println!("Found room {}", room.id);
                room.id
            }
            None => return Err(RoomNotFoundError),
        };
        let query = doc! { "username": user.username.clone(), "activated": false };
        user.activated = true;
        user.registered = Some(Utc::now());
        user.last_login = Some(Utc::now());
        user.in_room = Some(first_room_id);
        user.rooms_entered.push(first_room_id);
        user.pin = Option::default();
        let modification = doc! {
            "$set": {
                "activated": user.activated,
                "registered": Utc::now().timestamp() as u32,
                "last_login": Utc::now().timestamp() as u32,
                "in_room": first_room_id,
                "rooms_entered": &user.rooms_entered,
            },
            "$unset": {
                "pin": 0 as u32,
            },
        };
        match self
            .get_users_coll()
            .update_one(query, modification, None)
            .await
        {
            Ok(_) => {
                println!("Updated {}.", user.username);
            }
            Err(e) => {
                println!("Error: update failed ({:?})", e);
                return Err(MongoQueryError(e));
            }
        }
        Ok(())
    }
}

pub fn with_db(db: DB) -> impl Filter<Extract = (DB,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}
