/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use auth::{with_auth, Role};
use bson::oid::ObjectId;
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use db::{with_db, Direction, Password, PinType, Riddle, User, DB};
use futures::stream::StreamExt;
use lettre::{Message, SmtpTransport, Transport};
use mongodb_gridfs::{options::GridFSBucketOptions, GridFSBucket};
use pbkdf2::{
    password_hash::{Ident, PasswordHasher, SaltString},
    Algorithm, Params, Pbkdf2,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::From;
use std::env;
use std::net::SocketAddr;
use warp::{http::StatusCode, Filter, Rejection, Reply};

mod auth;
mod db;
mod error;
mod b64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let encoded = base64::encode(v);
        String::serialize(&encoded, s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let decoded = String::deserialize(d)?;
        base64::decode(decoded.as_bytes()).map_err(|e| serde::de::Error::custom(e))
    }
}

type Result<T> = std::result::Result<T, error::Error>;
type WebResult<T> = std::result::Result<T, Rejection>;

#[derive(Deserialize, Serialize, Debug)]
pub struct UserRegistrationRequest {
    pub username: String,
    pub email: String,
    pub role: String,
    pub password: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserActivationRequest {
    pub username: String,
    pub pin: PinType,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Debug)]
pub struct UserActivationResponse {
    pub jwt: String,
}

#[derive(Serialize, Debug)]
pub struct UserWhoamiResponse {
    pub username: String,
    pub email: String,
    pub role: String,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub registered: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(with = "ts_seconds_option")]
    pub last_login: Option<DateTime<Utc>>,
    pub level: u32,
    pub in_room: Option<ObjectId>,
    pub solved: Box<[Riddle]>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FileResponse {
    pub id: ObjectId,
    pub name: String,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Serialize, Debug)]
pub struct RiddleResponse {
    pub id: ObjectId,
    pub level: u32,
    pub files: Option<Vec<FileResponse>>,
    pub task: Option<String>,
    pub credits: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RoomResponse {
    pub id: ObjectId,
    pub neighbors: Box<[Direction]>,
    pub game_id: ObjectId,
    pub entry: Option<bool>,
    pub exit: Option<bool>,
}

fn empty_reply() -> warp::reply::Json {
    let empty: Vec<u8> = Vec::new();
    warp::reply::json(&empty)
}

pub async fn room_info_handler(id_str: String, username: String, db: DB) -> WebResult<impl Reply> {
    println!("room_info_handler called, id = {}", id_str);
    let oid = ObjectId::parse_str(id_str).unwrap();
    match db.get_user(&username).await {
        Ok(ref user) => {
            let _in_room = user.in_room;
        }
        Err(_) => {
            return Ok(warp::reply::with_status(
                empty_reply(),
                StatusCode::UNAUTHORIZED,
            ));
        }
    }
    match db.get_room_info(&oid).await {
        Ok(room) => {
            println!("got room {}", room.id);
            let reply = warp::reply::json(&json!(&RoomResponse {
                id: room.id,
                neighbors: room.neighbors,
                game_id: room.game_id,
                entry: room.entry,
                exit: room.exit,
            }));
            Ok(warp::reply::with_status(reply, StatusCode::OK))
        }
        Err(_) => Ok(warp::reply::with_status(
            empty_reply(),
            StatusCode::UNAUTHORIZED,
        )),
    }
}

pub async fn riddle_get_by_level_handler(
    level: u32,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("riddle_get_by_level_handler called, level = {}", level);
    match db.get_riddle_by_level(level).await {
        Ok(riddle) => match riddle {
            Some(riddle) => {
                println!("got riddle {}", riddle.level);
                let mut found_files: Vec<FileResponse> = Vec::new();
                if let Some(files) = riddle.files {
                    for file in files.iter() {
                        println!("trying to load file {:?}", file);
                        let bucket = GridFSBucket::new(
                            db.get_database(),
                            Some(GridFSBucketOptions::default()),
                        );
                        let mut cursor = bucket.open_download_stream(file.file_id).await.unwrap();
                        let mut data: Vec<u8> = Vec::new();
                        while let Some(mut chunk) = cursor.next().await {
                            data.append(&mut chunk);
                        }
                        found_files.push(FileResponse {
                            id: file.file_id,
                            name: file.name.clone(),
                            data: data,
                            mime_type: file.mime_type.clone(),
                            width: file.width,
                            height: file.height,
                        })
                    }
                }
                let reply = warp::reply::json(&json!(&RiddleResponse {
                    id: riddle.id,
                    level: riddle.level,
                    files: Option::from(found_files),
                    task: Option::from(riddle.task),
                    credits: Option::from(riddle.credits),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                return Ok(reply);
            }
            None => return Ok(warp::reply::with_status(empty_reply(), StatusCode::OK)),
        },
        Err(_) => {
            return Ok(warp::reply::with_status(
                empty_reply(),
                StatusCode::UNAUTHORIZED,
            ));
        }
    }
}

pub async fn user_authentication_handler(username: String) -> WebResult<impl Reply> {
    println!(
        "user_authentication_handler called, username = {}",
        username
    );
    Ok(StatusCode::OK)
}

pub async fn user_whoami_handler(username: String, db: DB) -> WebResult<impl Reply> {
    println!("user_whoami_handler called, username = {}", username);
    match db.get_user(&username).await {
        Ok(user) => {
            println!("got user {} with email {}", user.username, user.email);
            let reply = warp::reply::json(&json!(&UserWhoamiResponse {
                username: user.username.clone(),
                email: user.email.clone(),
                role: user.role.clone(),
                created: Option::from(user.created),
                registered: Option::from(user.registered),
                last_login: Option::from(user.last_login),
                level: user.level,
                in_room: user.in_room,
                solved: user.solved,
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            return Ok(reply);
        }
        Err(_) => {
            let reply = warp::reply::with_status(empty_reply(), StatusCode::UNAUTHORIZED);
            return Ok(reply);
        }
    }
}

pub async fn user_login_handler(body: UserLoginRequest, mut db: DB) -> WebResult<impl Reply> {
    println!("user_login_handler called, username = {}", body.username);
    match db.get_user(&body.username).await {
        Ok(user) => {
            println!(
                "got user {} with salt {}",
                user.username,
                user.password.clone().unwrap().salt.as_str()
            );
            let salt = SaltString::new(user.password.clone().unwrap().salt.as_str()).unwrap();
            // TODO: deduplicate code (see user_registration_handler())
            let password_hash = Pbkdf2
                .hash_password_customized(
                    body.password.as_bytes(),
                    Some(Ident::new(Algorithm::Pbkdf2Sha256.as_str())),
                    None,
                    Params {
                        rounds: 10000,
                        output_length: 32,
                    },
                    &salt,
                )
                .unwrap()
                .to_string();
            if password_hash == user.password.as_ref().unwrap().hash {
                println!("Hashes match. User is verified.");
                let _ = db.login_user(&user).await;
                let token_str = auth::create_jwt(&user.username, &Role::from_str(&user.role));
                let reply = warp::reply::json(&json!(&UserActivationResponse {
                    jwt: token_str.unwrap(),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                return Ok(reply);
            }
        }
        Err(_) => {
            let reply = warp::reply::with_status(empty_reply(), StatusCode::UNAUTHORIZED);
            return Ok(reply);
        }
    }
    let reply = warp::reply::with_status(empty_reply(), StatusCode::UNAUTHORIZED);
    Ok(reply)
}

pub async fn user_activation_handler(
    body: UserActivationRequest,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!(
        "user_activation_handler called, username was: {}, pin was: {}",
        body.username, body.pin
    );
    let user = db.get_user_with_pin(&body.username, body.pin).await;
    match user {
        Ok(user) => {
            let _ = db.activate_user(&user).await;
            let token_str = auth::create_jwt(&user.username, &Role::from_str(&user.role));
            let reply = warp::reply::json(&json!(&UserActivationResponse {
                jwt: token_str.unwrap(),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
        Err(_) => {
            let reply = warp::reply::with_status(empty_reply(), StatusCode::FORBIDDEN);
            Ok(reply)
        }
    }
}

pub async fn user_registration_handler(
    body: UserRegistrationRequest,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!(
        "user_register_handler called, username was: {}, email was: {}, role was: {}, password was: {}",
        body.username, body.email, body.role, body.password
    );
    let user = db.get_user(&body.username).await;
    match user {
        Ok(_) => {
            let reply = warp::reply::with_status(empty_reply(), StatusCode::FORBIDDEN);
            Ok(reply)
        }
        Err(_) => {
            // generate salt, then run password through PBKDF2
            let salt = SaltString::generate(&mut OsRng);
            let password_hash = Pbkdf2
                .hash_password_customized(
                    body.password.as_bytes(),
                    Some(Ident::new(Algorithm::Pbkdf2Sha256.as_str())),
                    None,
                    Params {
                        rounds: 10000,
                        output_length: 32,
                    },
                    &salt,
                )
                .unwrap()
                .to_string();
            println!("SALT: {}\nHASH: {}", salt.as_str(), password_hash);
            let password: Option<Password> = Option::from(Password::new(
                &String::from(salt.as_str()),
                &String::from(password_hash),
            ));
            let pin: PinType = OsRng.next_u32() % 1000000;
            let _result = db
                .create_user(&User::new(
                    &body.username,
                    &body.email,
                    &body.role,
                    password,
                    Option::from(pin),
                    false,
                    Option::from(Utc::now()),
                    Option::default(),
                    Option::default(),
                    Box::new([]),
                    0,
                    Option::default(),
                ))
                .await;
            let email = Message::builder()
                .from(
                    "Labyrinth Mailer <no-reply@ersatzworld.net>"
                        .parse()
                        .unwrap(),
                )
                .to(format!("{} <{}>", body.username, body.email)
                    .parse()
                    .unwrap())
                .date_now()
                .subject("Your Labyrinth Activation PIN")
                .body(format!(
                    r#"Hi {}!

You've successfully registered with Labyrinth.

Your PIN: {:06}

Now go back to the Labyrinth website and enter it to activate your account.

Cheers,
Your Labyrinth Host


*** If you don't know what this mail is about, please ignore it ;-)"#,
                    body.username, pin
                ))
                .unwrap();
            let mailer = SmtpTransport::unencrypted_localhost();
            match mailer.send(&email) {
                Ok(_) => {
                    println!(
                        "Mail with PIN {:06} successfully sent to {} <{}>.",
                        pin, body.username, body.email
                    );
                }
                Err(e) => {
                    println!(
                        "Error sending mail to {} <{}>: {:?}",
                        body.username, body.email, e
                    );
                }
            }
            let reply = warp::reply::with_status(empty_reply(), StatusCode::OK);
            Ok(reply)
        }
    }
}

pub async fn null_handler() -> WebResult<impl Reply> {
    println!("null_handler called",);
    Ok(StatusCode::OK)
}

pub async fn u32_handler(_: u32) -> WebResult<impl Reply> {
    println!("u32_handler called",);
    Ok(StatusCode::OK)
}

pub async fn string_handler(_: String) -> WebResult<impl Reply> {
    println!("string_handler called",);
    Ok(StatusCode::OK)
}

#[tokio::main]
async fn main() -> Result<()> {
    let db = DB::init().await?;
    /*
    let mut headers = HeaderMap::new();
    headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    headers.insert(
        "Access-Control-Allow-Headers",
        HeaderValue::from_static("x-csrf-token,authorization,content-type,accept,origin,x-requested-with,access-control-allow-origin"));
    headers.insert("Allow-Credentials", HeaderValue::from_static("true"));
    headers.insert(
        "Allow-Methods",
        HeaderValue::from_static("GET,POST,PUT,PATCH,OPTIONS,DELETE"),
    );
    */
    let root = warp::path::end().map(|| "Labyrinth API root.");
    /*
    let cors = warp::cors()
        .max_age(60 * 60 * 24 * 30)
        .allow_any_origin()
        .allow_credentials(true)
        .allow_methods(&[
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
            Method::PATCH,
            Method::PUT,
        ]);
    let cors_route = warp::any().map(warp::reply).with(&cors);
    */
    let ping_route = warp::path!("ping").and(warp::get()).map(warp::reply);
    let user_register_route = warp::path!("user" / "register")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_registration_handler);
    let user_activation_route = warp::path!("user" / "activate")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_activation_handler);
    let user_login_route = warp::path!("user" / "login")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_login_handler);
    let user_auth_route = warp::path!("user" / "auth")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and_then(user_authentication_handler);
    let user_whoami_route = warp::path!("user" / "whoami")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(user_whoami_handler);
    let riddle_get_by_level_route = warp::path!("riddle" / "by" / "level" / u32)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(riddle_get_by_level_handler);
    let room_info_route = warp::path!("room" / "info" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(room_info_handler);

    let routes = root
        .or(room_info_route)
        .or(riddle_get_by_level_route)
        .or(user_whoami_route)
        .or(user_auth_route)
        .or(user_login_route)
        .or(user_register_route)
        .or(user_activation_route)
        .or(ping_route)
        .or(warp::any().and(warp::options()).map(warp::reply));
    //.recover(error::handle_rejection);

    let host = env::var("API_HOST").expect("API_HOST is not in .env file");
    let addr: SocketAddr = host.parse().expect("Cannot parse host address");
    println!("Listening on http://{}", host);
    warp::serve(routes).run(addr).await;
    Ok(())
}
