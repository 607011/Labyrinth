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
    pub name: String,
    pub scale: Option<u32>,
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
    pub level: u32,
    pub files: Option<Vec<UploadedFile>>,
    pub solution: String,
    pub task: Option<String>,
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
    pub number: u32,
    pub neighbors: Vec<Direction>,
    pub game_id: ObjectId,
    pub entry: Option<bool>,
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
    pub level: u32,
    pub in_room: Option<ObjectId>,
}

impl User {
    pub fn new(
        username: &String,
        email: &String,
        role: Role,
        hash: String,
        pin: Option<PinType>,
        activated: bool,
        created: Option<DateTime<Utc>>,
        registered: Option<DateTime<Utc>>,
        last_login: Option<DateTime<Utc>>,
        solved: Vec<ObjectId>,
        level: u32,
        in_room: Option<ObjectId>,
    ) -> Self {
        User {
            id: ObjectId::new(),
            username: username.to_string(),
            email: email.to_string(),
            role: role,
            hash: hash,
            pin: pin,
            activated: activated,
            created: created,
            registered: registered,
            last_login: last_login,
            solved: solved,
            level: level,
            in_room: in_room,
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

    pub async fn get_riddle_by_level(&self, level: u32) -> Result<Option<Riddle>> {
        println!("get_riddle_by_level(\"{}\")", level);
        let coll = self.get_riddles_coll();
        let result = coll.find_one(doc! { "level": level }, None).await.unwrap();
        match result {
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

    pub async fn get_riddle_by_oid(&self, oid: ObjectId) -> Result<Option<Riddle>> {
        println!("get_riddle_by_oid(\"{:?}\")", oid);
        let coll = self.get_riddles_coll();
        let result = coll.find_one(doc! { "_id": oid }, None).await.unwrap();
        match result {
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

    pub async fn is_riddle_accessible(
        &self,
        oid: &ObjectId,
        username: &String,
    ) -> (Option<ObjectId>, Option<User>, Option<String>) {
        // get the user associated with the request
        let user = match self.get_user(&username).await {
            Ok(user) => user,
            Err(_) => {
                return (
                    Option::default(),
                    Option::default(),
                    Option::from("either username or password is wrong".to_string()),
                );
            }
        };
        // get the ID of the room the user is in
        let in_room = match user.in_room {
            Some(in_room) => in_room,
            None => {
                return (
                    Option::default(),
                    Option::default(),
                    Option::from("User is nowhere. That should not have happened :-/".to_string()),
                );
            }
        };
        // get the room
        let room = match self.get_room(&in_room).await {
            Ok(room) => room,
            Err(_) => {
                return (
                    Option::default(),
                    Option::default(),
                    Option::from("room not found".to_string()),
                );
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
                    Option::from("doorway not accessible".to_string()),
                );
            }
        };
        (
            Option::from(found.riddle_id),
            Option::from(user),
            Option::default(),
        )
    }

    pub async fn get_user(&self, username: &String) -> Result<User> {
        println!("get_user(\"{}\")", username);
        let coll = self.get_users_coll();
        let result = coll
            .find_one(doc! { "username": username }, None)
            .await
            .unwrap();
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

    pub async fn get_room(&self, oid: &ObjectId) -> Result<Room> {
        println!("get_room(\"{}\")", oid);
        let coll = self.get_rooms_coll();
        let result = coll.find_one(doc! { "_id": oid }, None).await.unwrap();
        match result {
            Some(room) => {
                println!("Found {}", room.id);
                Ok(room)
            }
            None => {
                println!("room not found");
                Err(RoomNotFoundError)
            }
        }
    }

    pub async fn get_user_with_pin(&self, username: &String, pin: PinType) -> Result<User> {
        println!("get_user_with_pin(\"{}\", \"{:06}\")", username, pin);
        let coll = self.get_users_coll();
        let result = coll
            .find_one(
                doc! { "username": username, "pin": pin, "activated": false },
                None,
            )
            .await
            .unwrap();
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

    /*
    pub async fn get_user_with_password(
        &self,
        username: &String,
        password: &String,
    ) -> Result<User> {
        println!("get_user_with_password(\"{}\", \"{}\")", username, password);
        let coll = self.get_users_coll();
        let result = coll
            .find_one(
                doc! { "username": username, "password": password, "activated": false },
                None,
            )
            .await
            .unwrap();
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
    */

    pub async fn create_user(&mut self, user: &User) -> Result<()> {
        self.get_users_coll()
            .insert_one(user, None)
            .await
            .map_err(MongoQueryError)?;
        Ok(())
    }

    pub async fn login_user(&mut self, user: &User) -> Result<()> {
        match self
            .get_users_coll()
            .update_one(
                doc! { "username": user.username.clone(), "activated": true },
                doc! {
                    "$set": { "last_login": Option::from(Utc::now().timestamp()) },
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
                Err(UserUpdateError)
            }
        }
    }

    pub async fn activate_user(&mut self, user: &mut User) -> Result<()> {
        let entrance: Option<Room> = self
            .get_rooms_coll()
            .find_one(
                doc! {
                    "entry": true,
                    /* XXX: choose a game_id */
                },
                None,
            )
            .await
            .unwrap();
        let first_room_id = match entrance {
            Some(ref room) => {
                println!("Found room {}", room.id);
                Option::from(room.id)
            }
            None => {
                println!("room not found");
                None
            }
        };
        let query = doc! { "username": user.username.clone(), "activated": false };
        user.activated = true;
        user.registered = Option::from(Utc::now());
        user.last_login = Option::from(Utc::now());
        user.in_room = first_room_id;
        user.pin = Option::default();
        let modification = doc! {
            "$set": {
                "activated": user.activated,
                "registered": Utc::now().timestamp() as u32,
                "last_login": Utc::now().timestamp() as u32,
                "in_room": first_room_id.unwrap(),
            },
            "$unset": {
                "pin": 0 as u32,
            },
        };
        let result = self
            .get_users_coll()
            .update_one(query, modification, None)
            .await;
        match result {
            Ok(_) => {
                println!("Updated {}.", user.username);
            }
            Err(e) => {
                println!("Error: update failed ({:?})", e);
            }
        }
        Ok(())
    }
}

pub fn with_db(db: DB) -> impl Filter<Extract = (DB,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}
