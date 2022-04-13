/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use crate::error::Error;
use argon2::{self, Config, ThreadMode, Variant, Version};
use auth::{with_auth, Role};
use bson::oid::ObjectId;
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use db::{with_db, Direction, PinType, User, DB};
use dotenv::dotenv;
use futures::stream::StreamExt;
use lazy_static::lazy_static;
use lettre::{Message, SmtpTransport, Transport};
use mongodb::bson::doc;
use mongodb_gridfs::{options::GridFSBucketOptions, GridFSBucket};
use rand;
use rand_core::{OsRng, RngCore};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::convert::From;
use std::env;
use std::io::Read;
use std::net::SocketAddr;
use url_escape;
use warp::{http::StatusCode, reject, reply::WithStatus, Filter, Rejection, Reply};

mod auth;
mod b64;
mod db;
mod error;

type Result<T> = std::result::Result<T, error::Error>;
type WebResult<T> = std::result::Result<T, Rejection>;
type OidString = String;

lazy_static! {
    static ref BAD_HASHES: Vec<Vec<u8>> = {
        let file = &std::fs::File::open("toppass8-md5.bin").unwrap();
        let chunk_size: usize = 128 / 8;
        let mut hashes: Vec<Vec<u8>> = Vec::new();
        loop {
            let mut chunk = Vec::with_capacity(chunk_size);
            let n = file
                .take(chunk_size as u64)
                .read_to_end(&mut chunk)
                .unwrap();
            if n == 0 {
                break;
            }
            hashes.push(chunk);
            if n < chunk_size {
                break;
            }
        }
        hashes
    };
    static ref OPPOSITE: HashMap<String, String> = HashMap::from([
        (String::from("n"), String::from("s")),
        (String::from("e"), String::from("w")),
        (String::from("s"), String::from("n")),
        (String::from("w"), String::from("e")),
        //(String::from("u"), String::from("d")),
        //(String::from("d"), String::from("u")),
    ]);
    static ref RE_MAIL: Regex = Regex::new(r"(^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$)").unwrap();
}

fn bad_password(password: &String) -> bool {
    let hash = md5::compute(password.as_bytes());
    match BAD_HASHES.binary_search(&Vec::from(*hash)) {
        Ok(_hash) => true,
        _ => false,
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserRegistrationRequest {
    pub username: String,
    pub email: String,
    pub role: Role,
    pub password: String,
    #[serde(default)]
    pub locale: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct StatusResponse {
    pub ok: bool,
    pub message: Option<String>,
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

#[derive(Deserialize, Serialize, Debug)]
pub struct RoomResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub id: ObjectId,
    pub number: u32,
    pub coords: Option<String>,
    pub neighbors: Vec<Direction>,
    pub game_id: ObjectId,
    pub entry: Option<bool>,
    pub exit: Option<bool>,
}

impl RoomResponse {
    pub fn bad_with_message(message: Option<String>) -> RoomResponse {
        RoomResponse {
            ok: false,
            message: Some(message.unwrap_or("room not found".to_string())),
            id: bson::oid::ObjectId::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            number: 0,
            coords: Option::default(),
            neighbors: Vec::new(),
            game_id: bson::oid::ObjectId::from_bytes([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            entry: Option::default(),
            exit: Option::default(),
        }
    }
    pub fn bad() -> RoomResponse {
        RoomResponse::bad_with_message(Option::default())
    }
}

#[derive(Serialize, Debug)]
pub struct UserWhoamiResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub username: String,
    pub email: String,
    pub role: Role,
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
    pub level: u32,
    pub score: u32,
    pub in_room: RoomResponse,
    pub solved: Vec<ObjectId>,
    pub rooms_entered: Vec<ObjectId>,
    pub jwt: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FileVariantResponse {
    pub name: String,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    pub scale: Option<u32>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FileResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub id: ObjectId,
    pub name: String,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub scale: Option<u32>,
    pub variants: Option<Vec<FileVariantResponse>>,
}

#[derive(Serialize, Debug)]
pub struct RiddleResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub id: ObjectId,
    pub level: u32,
    pub files: Option<Vec<FileResponse>>,
    pub task: Option<String>,
    pub difficulty: u32,
    pub deduction: u32,
    pub credits: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct DebriefingResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub debriefing: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct RiddleSolvedResponse {
    pub ok: bool,
    pub riddle_id: ObjectId,
    pub solved: bool,
    pub score: u32,
    pub level: u32,
    pub message: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct SteppedThroughResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub room: RoomResponse,
}

#[derive(Serialize, Debug)]
pub struct GameStatsResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub num_rooms: u32,
    pub num_riddles: u32,
    pub max_score: u32,
}

fn err_response(message: Option<String>) -> WithStatus<warp::reply::Json> {
    let reply = warp::reply::json(&json!(&StatusResponse {
        ok: false,
        message: message,
    }));
    warp::reply::with_status(reply, StatusCode::OK)
}

pub async fn go_handler(direction_str: String, username: String, db: DB) -> WebResult<impl Reply> {
    println!(
        "go_handler called, direction = {}, username = {}",
        direction_str, username
    );
    let mut user = match db.get_user(&username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    if user.in_room.is_none() {
        return Err(reject::custom(Error::UserIsInNoRoom));
    }
    let room = match db.get_room(&user.in_room.unwrap()).await {
        Ok(room) => {
            println!("current room: {}", room.id);
            room
        }
        Err(e) => return Err(reject::custom(e)),
    };
    let direction = match room
        .neighbors
        .iter()
        .find(|&neighbor| neighbor.direction == direction_str)
    {
        Some(direction) => {
            println!(
                "riddle in direction {}: {}",
                direction_str, direction.riddle_id
            );
            direction
        }
        None => return Err(reject::custom(Error::NeighborNotFoundError)),
    };
    let riddle_id = match user.solved.iter().find(|&&s| s == direction.riddle_id) {
        Some(riddle_id) => riddle_id,
        None => return Err(reject::custom(Error::RiddleNotSolvedError)),
    };
    let opposite = &OPPOSITE[&direction.direction];
    let room_behind = match db.get_room_behind(&opposite, &riddle_id).await {
        Ok(room_behind) => room_behind,
        Err(e) => return Err(reject::custom(e)),
    };
    println!(
        "moving from room {} to {}",
        &user.in_room.unwrap(),
        &room_behind.id
    );
    user.in_room = Some(room_behind.id);
    // TODO: move all code accessing the database to db.rs
    let update_doc = match room.exit.is_some() && room.exit.unwrap() {
        true => doc! {
            "$set": {
                "in_room": user.in_room,
            },
            "$addToSet": { "rooms_entered": user.in_room },
            "$addToSet": {
                "finished": {
                    "game_id": room.game_id,
                    "timestamp": Utc::now().timestamp() as u32,
                }
            }
        },
        false => doc! {
            "$set": {
                "in_room": user.in_room,
            },
            "$addToSet": { "rooms_entered": user.in_room },
        },
    };
    match db
        .get_users_coll()
        .update_one(doc! { "_id": user.id, "activated": true }, update_doc, None)
        .await
    {
        Ok(_) => {}
        Err(e) => return Ok(err_response(Some(e.to_string()))),
    };
    let room = match db.get_room(&user.in_room.unwrap()).await {
        Ok(room) => {
            println!("new room: {}", room.id);
            room
        }
        Err(e) => return Ok(err_response(Some(e.to_string()))),
    };
    let reply = warp::reply::json(&json!(&SteppedThroughResponse {
        ok: true,
        message: Option::default(),
        room: RoomResponse {
            ok: true,
            message: Option::default(),
            id: room.id,
            number: room.number,
            coords: room.coords,
            entry: room.entry,
            exit: room.exit,
            game_id: room.game_id,
            neighbors: room.neighbors,
        },
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

pub async fn riddle_solve_handler(
    riddle_id_str: String,
    solution: String,
    username: String,
    mut db: DB,
) -> WebResult<impl Reply> {
    let solution = url_escape::decode(&solution);
    println!(
        "riddle_solve_handler called, oid = {}, solution = {}",
        riddle_id_str, solution
    );
    let oid = match ObjectId::parse_str(&riddle_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let (riddle_id, user, _msg) = db.is_riddle_accessible(&oid, &username).await;
    if riddle_id.is_none() {
        return Err(reject::custom(Error::RiddleNotFoundError));
    }
    let riddle = match db.get_riddle_by_oid(&riddle_id.unwrap()).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle = match riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    let solved = match riddle.exact_match {
        true => riddle.solution == solution,
        false => riddle.solution.to_lowercase() == solution.to_lowercase(),
    };
    let mut user = match user {
        Some(user) => user,
        None => return Err(reject::custom(Error::UserNotFoundError)),
    };
    if solved {
        let mut solutions = user.solved.clone();
        solutions.push(riddle.id);
        user.level = riddle.level.max(user.level);
        user.score += riddle.difficulty;
        match db.set_user_solved(&solutions, &user).await {
            Ok(()) => {
                println!("User updated.");
            }
            Err(e) => {
                println!("Error: update failed ({:?})", e);
                return Err(reject::custom(Error::RiddleNotSolvedError));
            }
        }
    } else {
        user.score -= (riddle.difficulty / 2).max(1);
        match db.rewrite_user_score(&user).await {
            Ok(()) => {
                println!("User updated.");
            }
            Err(e) => {
                println!("Error: update failed ({:?})", e);
                return Err(reject::custom(Error::RiddleNotSolvedError));
            }
        }
    }
    let reply = warp::reply::json(&json!(&RiddleSolvedResponse {
        ok: true,
        riddle_id: riddle.id,
        solved: solved,
        score: user.score,
        level: riddle.level,
        message: Option::default(),
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

pub async fn debriefing_get_by_riddle_id_handler(
    riddle_id_str: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!(
        "debriefing_get_by_riddle_id_handler called, oid = {}",
        riddle_id_str
    );
    let oid = match ObjectId::parse_str(riddle_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let solved_riddle = match db.get_riddle_if_solved(&oid, &username).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle = match solved_riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    println!("got riddle {}", riddle.level);
    let reply = warp::reply::json(&json!(&DebriefingResponse {
        ok: true,
        message: Option::default(),
        debriefing: riddle.debriefing,
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

pub async fn riddle_get_oid_handler(
    riddle_id_str: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("riddle_get_oid_handler called, oid = {}", riddle_id_str);
    let oid = match ObjectId::parse_str(riddle_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let (riddle_id, _user, message) = db.is_riddle_accessible(&oid, &username).await;
    let riddle_id = match riddle_id {
        Some(riddle_id) => riddle_id,
        None => return Ok(err_response(message)),
    };
    let riddle = match db.get_riddle_by_oid(&riddle_id).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle = match riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    println!("got riddle {}", riddle.level);
    let mut found_files: Vec<FileResponse> = Vec::new();
    if let Some(files) = riddle.files {
        for file in files.iter() {
            println!("trying to load file {:?}", file);
            let bucket = GridFSBucket::new(db.get_database(), Some(GridFSBucketOptions::default()));
            let mut cursor = match bucket.open_download_stream(file.file_id).await {
                Ok(cursor) => cursor,
                Err(e) => return Err(reject::custom(Error::GridFSError(e))),
            };
            let mut data: Vec<u8> = Vec::new();
            while let Some(mut chunk) = cursor.next().await {
                data.append(&mut chunk);
            }
            let mut file_variants: Vec<FileVariantResponse> = Vec::new();
            if let Some(variants) = &file.variants {
                for variant in variants {
                    let bucket =
                        GridFSBucket::new(db.get_database(), Some(GridFSBucketOptions::default()));
                    let mut cursor = match bucket.open_download_stream(variant.file_id).await {
                        Ok(cursor) => cursor,
                        Err(e) => return Err(reject::custom(Error::GridFSError(e))),
                    };
                    let mut data: Vec<u8> = Vec::new();
                    while let Some(mut chunk) = cursor.next().await {
                        data.append(&mut chunk);
                    }
                    file_variants.push(FileVariantResponse {
                        name: variant.name.clone(),
                        data: data,
                        scale: Some(variant.scale),
                    });
                }
            }
            found_files.push(FileResponse {
                ok: true,
                message: Option::default(),
                id: file.file_id,
                name: file.name.clone(),
                data: data,
                mime_type: file.mime_type.clone(),
                scale: file.scale,
                width: file.width,
                height: file.height,
                variants: Some(file_variants),
            })
        }
    }
    let reply = warp::reply::json(&json!(&RiddleResponse {
        ok: true,
        message: Option::default(),
        id: riddle.id,
        level: riddle.level,
        difficulty: riddle.difficulty,
        deduction: riddle.deduction.unwrap_or(0),
        files: Option::from(found_files),
        task: riddle.task,
        credits: riddle.credits,
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

pub async fn game_stats_handler(
    game_id_str: String,
    _username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("game_stats_handler called, game_id = {}", game_id_str);
    let game_id = match ObjectId::parse_str(game_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let num_rooms = match db.get_num_rooms(&game_id).await {
        Ok(num_rooms) => num_rooms,
        Err(e) => return Err(reject::custom(e)),
    };
    let num_riddles = match db.get_num_riddles(&game_id).await {
        Ok(num_riddles) => num_riddles,
        Err(e) => return Err(reject::custom(e)),
    };
    let reply = warp::reply::json(&json!(&GameStatsResponse {
        ok: true,
        message: Option::default(),
        num_rooms: num_rooms.unwrap(),
        num_riddles: num_riddles.unwrap(), // TODO
        max_score: 0,                      // TODO
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

// This function is needed for manual debugging.
pub async fn riddle_get_by_level_handler(
    level: u32,
    _username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("riddle_get_by_level_handler called, level = {}", level);
    let riddle = match db.get_riddle_by_level(level).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle = match riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    println!("got riddle {}", riddle.level);
    let mut found_files: Vec<FileResponse> = Vec::new();
    if let Some(files) = riddle.files {
        for file in files.iter() {
            println!("trying to load file {:?}", file);
            let bucket = GridFSBucket::new(db.get_database(), Some(GridFSBucketOptions::default()));
            let mut cursor = match bucket.open_download_stream(file.file_id).await {
                Ok(cursor) => cursor,
                Err(e) => return Err(reject::custom(Error::GridFSError(e))),
            };
            let mut data: Vec<u8> = Vec::new();
            while let Some(mut chunk) = cursor.next().await {
                data.append(&mut chunk);
            }
            found_files.push(FileResponse {
                ok: true,
                message: Option::default(),
                id: file.file_id,
                name: file.name.clone(),
                data: data,
                mime_type: file.mime_type.clone(),
                scale: file.scale,
                width: file.width,
                height: file.height,
                variants: Option::default(),
            });
        }
    }
    let reply = warp::reply::json(&json!(&RiddleResponse {
        ok: true,
        message: Option::default(),
        id: riddle.id,
        level: riddle.level,
        difficulty: riddle.difficulty,
        deduction: riddle.deduction.unwrap_or(0),
        files: Option::from(found_files),
        task: riddle.task,
        credits: riddle.credits,
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

pub async fn user_authentication_handler(username: String) -> WebResult<impl Reply> {
    println!(
        "user_authentication_handler called, username = {}",
        username
    );
    Ok(StatusCode::OK)
}

pub async fn cheat_handler(username: String) -> WebResult<impl Reply> {
    println!("cheat_handler called, username = {}", username);
    if true {
        return Err(reject::custom(Error::CheatError));
    }
    Ok(StatusCode::PAYMENT_REQUIRED)
}

pub async fn user_whoami_handler(username: String, db: DB) -> WebResult<impl Reply> {
    println!("user_whoami_handler called, username = {}", username);
    let user = match db.get_user(&username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    println!("got user {} with email {}", user.username, user.email);
    let in_room = match user.in_room {
        Some(room) => room,
        None => return Err(reject::custom(Error::RoomNotFoundError)),
    };
    let room_response = match db.get_room(&in_room).await {
        Ok(room) => RoomResponse {
            ok: true,
            message: Option::default(),
            id: room.id,
            number: room.number,
            coords: room.coords,
            neighbors: room.neighbors,
            game_id: room.game_id,
            entry: room.entry,
            exit: room.exit,
        },
        Err(e) => return Err(reject::custom(e)),
    };
    let reply = warp::reply::json(&json!(&UserWhoamiResponse {
        ok: true,
        message: Option::default(),
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role.clone(),
        activated: user.activated,
        created: user.created,
        registered: user.registered,
        last_login: user.last_login,
        level: user.level,
        score: user.score,
        in_room: room_response,
        solved: user.solved,
        rooms_entered: user.rooms_entered,
        jwt: Option::default(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_login_handler(body: UserLoginRequest, mut db: DB) -> WebResult<impl Reply> {
    println!("user_login_handler called, username = {}", body.username);
    let user = match db.get_user(&body.username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    println!("got user {}\nwith hash  = {}", user.username, user.hash);
    let matches = argon2::verify_encoded(&user.hash, body.password.as_bytes()).unwrap();
    if !matches {
        return Err(reject::custom(Error::WrongCredentialsError));
    }
    println!("Hashes match. User is verified.");
    match db.login_user(&user).await {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    }
    let token_str = match auth::create_jwt(&user.username, &user.role) {
        Ok(token_str) => token_str,
        Err(e) => return Err(reject::custom(e)),
    };
    let room_response = match db.get_room(&user.in_room.unwrap()).await {
        Ok(room) => RoomResponse {
            ok: true,
            message: Option::default(),
            id: room.id,
            number: room.number,
            coords: room.coords,
            neighbors: room.neighbors,
            game_id: room.game_id,
            entry: room.entry,
            exit: room.exit,
        },
        Err(e) => return Err(reject::custom(e)),
    };
    let reply = warp::reply::json(&json!(&UserWhoamiResponse {
        ok: true,
        message: Option::default(),
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role.clone(),
        activated: user.activated,
        created: user.created,
        registered: user.registered,
        last_login: user.last_login,
        level: user.level,
        score: user.score,
        in_room: room_response,
        solved: user.solved,
        rooms_entered: user.rooms_entered,
        jwt: Some(token_str),
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
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
    let mut user = match db.get_user_with_pin(&body.username, body.pin).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    match db.activate_user(&mut user).await {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    };
    let token_str = match auth::create_jwt(&user.username, &user.role) {
        Ok(token_str) => token_str,
        Err(e) => return Err(reject::custom(e)),
    };
    let room_response = match db.get_room(&user.in_room.unwrap()).await {
        Ok(room) => RoomResponse {
            ok: true,
            message: Option::default(),
            id: room.id,
            number: room.number,
            coords: room.coords,
            neighbors: room.neighbors,
            game_id: room.game_id,
            entry: room.entry,
            exit: room.exit,
        },
        Err(e) => return Err(reject::custom(e)),
    };
    let reply = warp::reply::json(&json!(&UserWhoamiResponse {
        ok: true,
        message: Option::default(),
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role.clone(),
        activated: user.activated,
        created: user.created,
        registered: user.registered,
        last_login: user.last_login,
        level: user.level,
        score: user.score,
        in_room: room_response,
        solved: user.solved,
        rooms_entered: user.rooms_entered,
        jwt: Some(token_str),
    }));
    let reply = warp::reply::with_status(reply, StatusCode::OK);
    Ok(reply)
}

pub async fn user_registration_handler(
    body: UserRegistrationRequest,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!(
        "user_register_handler called, username was: {}, email was: {}, role was: {}, password was: {}",
        body.username, body.email, body.role, body.password
    );
    if body.password.len() < 8 || bad_password(&body.password) {
        return Err(reject::custom(Error::UnsafePasswordError));
    }
    if !RE_MAIL.is_match(body.email.as_str()) {
        return Err(reject::custom(Error::InvalidEmailError));
    }
    if db.get_user(&body.username).await.is_ok() {
        return Err(reject::custom(Error::UsernameNotAvailableError));
    }
    let config = Config {
        variant: Variant::Argon2i,
        version: Version::Version13,
        mem_cost: 65536,
        time_cost: 10,
        lanes: 4,
        thread_mode: ThreadMode::Parallel,
        secret: &[],
        ad: &[],
        hash_length: 32,
    };
    let salt: Vec<u8> = (0..16).map(|_| rand::random::<u8>()).collect();
    let hash = argon2::hash_encoded(body.password.as_bytes(), &salt, &config).unwrap();
    let mut pin: PinType = 0;
    while pin == 0 {
        pin = OsRng.next_u32() % 1000000;
    }
    match db
        .create_user(&User::new(
            &body.username,
            &body.email,
            body.role,
            hash,
            Some(pin),
        ))
        .await
    {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    }
    let to = match format!("{} <{}>", body.username, body.email).parse() {
        Ok(to) => to,
        Err(_) => return Err(reject::custom(Error::MalformedAddressError)), // TODO: propagate info of `lettre::address::AddressError`
    };
    let email = match Message::builder()
        .from(
            "Labyrinth Mailer <no-reply@ersatzworld.net>"
                .parse()
                .unwrap(),
        )
        .to(to)
        .date_now()
        .subject("Deine Aktivierungs-PIN für Labyrinth")
        .body(format!(
            r#"Moin {}!

Du hast dich erfolgreich bei Labyrinth rrgistriert.

Deine PIN zur Aktivierung des Accounts: {:06}

Bitte gib diese PIN auf der Labyrinth-Website ein.

Viele Grüße,,
Dein Labyrinth-Betreuer


*** Falls du keinen Schimmer hast, was es mit dieser Mail auf sich hat, kannst du sie getrost ignorieren ;-)"#,
            body.username, pin
        )) {
        Ok(email) => email,
        Err(_) => return Err(reject::custom(Error::MailBuilderError)), // TODO: propagate info of `lettre::error::Error`
    };
    let mailer = SmtpTransport::unencrypted_localhost();
    match mailer.send(&email) {
        Ok(_) => {
            println!(
                "Mail with PIN {:06} successfully sent to {} <{}>.",
                pin, body.username, body.email
            );
        }
        Err(_) => return Err(reject::custom(Error::SmtpTransportError)), // TODO: propagate info of `lettre::transport::smtp::Error`
    }
    let reply = warp::reply::json(&json!(&StatusResponse {
        ok: true,
        message: Option::default(),
    }));
    let reply = warp::reply::with_status(reply, StatusCode::CREATED);
    Ok(reply)
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let db = DB::init().await?;
    let root = warp::path::end().map(|| "Labyrinth API root.");
    /* Routes accessible to all users */
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
    /* Routes accessible only to authorized users */
    let user_auth_route = warp::path!("user" / "auth")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and_then(user_authentication_handler);
    let user_whoami_route = warp::path!("user" / "whoami")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(user_whoami_handler);
    let riddle_get_by_oid_route = warp::path!("riddle" / OidString)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(riddle_get_oid_handler);
    let debriefing_get_by_riddle_id_route = warp::path!("debriefing" / OidString)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(debriefing_get_by_riddle_id_handler);
    let riddle_solve_route = warp::path!("riddle" / "solve" / OidString / "with" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(riddle_solve_handler);
    let go_route = warp::path!("go" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(go_handler);
    let game_stats_route = warp::path!("game" / "stats" / OidString)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(game_stats_handler);
    let cheat_route = warp::path!("cheat")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and_then(cheat_handler);
    /* Routes accessible only to authorized admins */
    let riddle_get_by_level_route = warp::path!("riddle" / "by" / "level" / u32)
        .and(warp::get())
        .and(with_auth(Role::Admin))
        .and(with_db(db.clone()))
        .and_then(riddle_get_by_level_handler);

    let routes = root
        .or(riddle_get_by_oid_route)
        .or(debriefing_get_by_riddle_id_route)
        .or(riddle_get_by_level_route)
        .or(riddle_solve_route)
        .or(go_route)
        .or(user_whoami_route)
        .or(user_auth_route)
        .or(user_login_route)
        .or(user_register_route)
        .or(user_activation_route)
        .or(ping_route)
        .or(cheat_route)
        .or(game_stats_route)
        .or(warp::any().and(warp::options()).map(warp::reply))
        .recover(error::handle_rejection);

    const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
    const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    println!("{} {}", CARGO_PKG_NAME, CARGO_PKG_VERSION);
    let host = env::var("API_HOST").expect("API_HOST is not in .env file");
    let addr: SocketAddr = host.parse().expect("Cannot parse host address");
    println!("Listening on http://{}", host);
    warp::serve(routes).run(addr).await;
    Ok(())
}
