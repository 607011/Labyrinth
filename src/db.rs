/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use crate::{error::Error::*, Result};
use bson::{oid::ObjectId, Bson};
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use mongodb::bson::doc;
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection, Database};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::env;
use warp::Filter;

pub type PinType = u32;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Password {
    pub salt: String,
    pub hash: String,
}

impl Password {
    pub fn new(salt: &String, hash: &String) -> Self {
        Password {
            salt: salt.to_string(),
            hash: hash.to_string(),
        }
    }
}

impl Into<Bson> for Password {
    fn into(self) -> bson::Bson {
        bson::Bson::Document(doc! {
            "salt": self.salt,
            "hash": self.hash,
        })
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UploadedFile {
    #[serde(rename = "fileId")]
    pub file_id: ObjectId,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Riddle {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub level: u32,
    pub files: Option<Box<[UploadedFile]>>,
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
pub struct Room {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub neighbors: Box<[Direction]>,
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
    pub role: String,
    pub password: Option<Password>,
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
    pub solved: Box<[Riddle]>,
    pub level: u32,
    pub in_room: Option<ObjectId>,
}

impl User {
    pub fn new(
        username: &String,
        email: &String,
        role: &String,
        password: Option<Password>,
        pin: Option<PinType>,
        activated: bool,
        created: Option<DateTime<Utc>>,
        registered: Option<DateTime<Utc>>,
        last_login: Option<DateTime<Utc>>,
        solved: Box<[Riddle]>,
        level: u32,
        in_room: Option<ObjectId>,
    ) -> Self {
        User {
            id: ObjectId::new(),
            username: username.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            password: password,
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

    fn get_users_coll(&self) -> Collection<User> {
        self.get_database().collection::<User>(&self.coll_users)
    }

    fn get_riddles_coll(&self) -> Collection<Riddle> {
        self.get_database().collection::<Riddle>(&self.coll_riddles)
    }

    fn get_rooms_coll(&self) -> Collection<Room> {
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

    pub async fn get_room_info(&self, oid: &ObjectId) -> Result<Room> {
        println!("get_room_info(\"{}\")", oid);
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

    pub async fn create_user(&mut self, user: &User) -> Result<()> {
        self.get_users_coll()
            .insert_one(user, None)
            .await
            .map_err(MongoQueryError)?;
        Ok(())
    }

    pub async fn login_user(&mut self, user: &User) -> Result<()> {
        let result = self
            .get_users_coll()
            .update_one(
                doc! { "username": user.username.clone(), "activated": true },
                doc! {
                    "$set": { "last_login": Option::from(Utc::now().timestamp()) },
                },
                None,
            )
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

    pub async fn activate_user(&mut self, user: &User) -> Result<()> {
        let first_room: Option<Room> = self
            .get_rooms_coll()
            .find_one(
                doc! { "_id": ObjectId::parse_str("6236fd5198083f2eb4fd6fb0").unwrap() },
                // doc! { "entry": true },
                None,
            )
            .await
            .unwrap();
        match first_room {
            Some(ref room) => {
                println!("Found room {}", room.id);
            }
            None => {
                println!("room not found");
                // Err(RoomNotFoundError)
            }
        }
        let query = doc! { "username": user.username.clone(), "activated": false };
        let modification = doc! {
            "$set": {
                "activated": true,
                "registered": Utc::now().timestamp() as u32,
                "last_login": Utc::now().timestamp() as u32,
                "in_room": first_room.unwrap().id,
            },
            "$unset": {
                "pin": 0,
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
