use crate::{error::Error::*, Result};
use bson::Bson;
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use mongodb::bson::doc;
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection, Database};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use warp::Filter;

pub type PinType = u32;
const DB_NAME: &str = "labyrinth";
const USERS_COLL: &str = "users";
const RIDDLES_COLL: &str = "riddles";
// const ROOMS_COLL: &str = "rooms";

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
pub struct User {
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
    ) -> Self {
        User {
            username: username.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            password: password,
            pin: pin,
            activated: activated,
            created: created,
            registered: registered,
            last_login: last_login,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UploadedFile {
    name: String,
    id: String,
    #[serde(rename(serialize = "mimeType", deserialize = "mimeType"))]
    mime_type: String,
    #[serde(rename(serialize = "originalFilename", deserialize = "originalFilename"))]
    original_filename: String,
    #[serde(rename(serialize = "webContentLink", deserialize = "webContentLink"))]
    url: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Riddle {
    pub level: u32,
    pub uploaded: Option<Box<[UploadedFile]>>,
    pub task: Option<String>,
    pub credits: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DB {
    pub client: Client,
}

impl DB {
    pub async fn init() -> Result<Self> {
        let mut client_options = ClientOptions::parse("mongodb://127.0.0.1:27017")
            .await
            .unwrap();
        client_options.app_name = Some(DB_NAME.to_string());
        Ok(Self {
            client: Client::with_options(client_options).unwrap(),
        })
    }

    fn get_database(&self) -> Database {
        self.client.database(DB_NAME)
    }

    fn get_users_coll(&self) -> Collection<User> {
        self.get_database().collection::<User>(USERS_COLL)
    }

    fn get_riddles_coll(&self) -> Collection<Riddle> {
        self.get_database().collection::<Riddle>(RIDDLES_COLL)
    }

    pub async fn get_riddle_by_level(&self, level: u32) -> Result<Riddle> {
        println!("get_riddle_by_level(\"{}\")", level);
        let coll = self.get_riddles_coll();
        let doc = doc! { "level": level };
        let result = coll.find_one(doc, None).await.unwrap();
        match result {
            Some(riddle) => {
                println!("Found {}", riddle.level);
                Ok(riddle)
            }
            None => {
                println!("riddle not found");
                Err(RiddleNotFoundError)
            }
        }
    }

    pub async fn get_user(&self, username: &String) -> Result<User> {
        println!("get_user(\"{}\")", username);
        let coll = self.get_users_coll();
        let doc = doc! { "username": username };
        let result = coll.find_one(doc, None).await.unwrap();
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

    pub async fn get_user_with_pin(&self, username: &String, pin: PinType) -> Result<User> {
        println!("get_user_with_pin(\"{}\", \"{:06}\")", username, pin);
        let coll = self.get_users_coll();
        let doc = doc! { "username": username, "pin": pin, "activated": false };
        let result = coll.find_one(doc, None).await.unwrap();
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

        let doc = doc! { "username": username, "password": password, "activated": false };
        let result = coll.find_one(doc, None).await.unwrap();
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
        let query = doc! { "username": user.username.clone(), "activated": true };
        let modification = doc! {
            "$set": { "last_login": Option::from(Utc::now().timestamp()) },
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

    pub async fn activate_user(&mut self, user: &User) -> Result<()> {
        let query = doc! { "username": user.username.clone(), "activated": false };
        let modification = doc! {
            "$set": {
                "activated": true,
                "registered": Utc::now().timestamp(),
                "last_login": Utc::now().timestamp(),
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
