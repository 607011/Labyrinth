/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use crate::error::Error;
use auth::{with_auth, Role};
use base32;
use bson::oid::ObjectId;
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use db::{with_db, Direction, PinType, Riddle, RiddleAttempt, Room, SecondFactor, User, DB};
use dotenv::dotenv;
use futures::stream::StreamExt;
use lazy_static::lazy_static;
use lettre::{Message, SmtpTransport, Transport};
use mongodb::bson::doc;
use mongodb_gridfs::{options::GridFSBucketOptions, GridFSBucket};
use passwd::Password;
use qrcode_generator::QrCodeEcc;
use rand::Rng;
use rand_core::{OsRng, RngCore};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::convert::From;
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use totp_lite::{totp_custom, Sha1};
use url_escape;
use warp::{http::StatusCode, reject, reply::WithStatus, Filter, Rejection, Reply};
use webauthn_rs::proto::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

mod auth;
mod b64;
mod db;
mod error;
mod passwd;
mod webauthn;

type Result<T> = std::result::Result<T, error::Error>;
type WebResult<T> = std::result::Result<T, Rejection>;
type OidString = String;

pub fn webauthn_default_config() -> webauthn::WebauthnVolatileConfig {
    let rp_name: String =
        env::var("RP_NAME").expect("environment variable RP_NAME has not been set");
    let rp_origin: String =
        env::var("RP_ORIGIN").expect("environment variable RP_ORIGIN has not been set");
    let rp_id: String = env::var("RP_ID").expect("environment variable RP_ID has not been set");
    let wa_config =
        webauthn::WebauthnVolatileConfig::new(&rp_name, &rp_origin, &rp_id, Option::default());
    wa_config
}

lazy_static! {
    static ref OPPOSITE: HashMap<String, String> = HashMap::from([
        (String::from("n"), String::from("s")),
        (String::from("e"), String::from("w")),
        (String::from("s"), String::from("n")),
        (String::from("w"), String::from("e")),
        //(String::from("u"), String::from("d")),
        //(String::from("d"), String::from("u")),
    ]);
    static ref RE_USERNAME: Regex = Regex::new(r"^\w+$").unwrap();
    static ref RE_MAIL: Regex = Regex::new(r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$").unwrap();
}

#[repr(C)]
union MD5Hash {
    hash: md5::Digest,
    value: u128,
}

fn is_bad_password(password: &String) -> std::result::Result<bool, std::io::Error> {
    let hash = MD5Hash {
        hash: md5::compute(password.as_bytes()),
    };
    let given_hash_raw = unsafe { MD5Hash { hash: hash.hash } };
    let given_hash = unsafe { u128::from_be(given_hash_raw.value) };
    let md5_filename = env::var("BAD_PASSWORDS_MD5")
        .expect("environment variable BAD_PASSWORDS_MD5 has not been set");
    let metadata = fs::metadata(&md5_filename).expect(&format!(
        "cannot read metadata of MD5 hash file '{}'",
        &md5_filename
    ));
    let mut lo: u64 = 0;
    let mut hi: u64 = metadata.len();
    const MD5_SIZE: u64 = 16;
    let mut f = &fs::File::open(&md5_filename)
        .expect(&format!("cannot read MD5 hash file '{}'", &md5_filename));
    let mut md5 = MD5Hash { value: 0 };
    while lo <= hi {
        let mut pos: u64 = (lo + hi) / 2;
        pos -= pos % MD5_SIZE;
        match f.seek(SeekFrom::Start(pos)) {
            Ok(_pos) => (),
            Err(e) => return Err(e),
        }
        unsafe {
            match f.read_exact(&mut *md5.hash) {
                Ok(_) => (),
                Err(e) => return Err(e),
            }
            let md5_value = u128::from_le(md5.value);
            if given_hash > md5_value {
                lo = pos + MD5_SIZE;
            } else if given_hash < md5_value {
                hi = pos - MD5_SIZE;
            } else {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[derive(Serialize, Debug)]
pub struct PingResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub version: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserRegistrationRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub locale: String,
    #[serde(rename = "secondFactorMethod")]
    pub second_factor: Option<SecondFactor>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct UserPasswordChangeRequest {
    pub password: String,
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

#[derive(Deserialize, Debug)]
pub struct UserLoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub totp: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct UserTotpRequest {
    pub username: String,
    pub totp: String,
}

#[derive(Deserialize, Debug)]
pub struct RiddleSolveRequest {
    pub solution: String,
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
pub struct TotpResponseRaw {
    #[serde(with = "b64")]
    pub qrcode: Vec<u8>,
    pub secret: String,
    pub hash: String,
    pub interval: u32,
    pub digits: u32,
}

impl TotpResponseRaw {
    pub fn new(qrcode: Vec<u8>, secret: String) -> TotpResponseRaw {
        TotpResponseRaw {
            qrcode,
            secret,
            hash: "SHA1".to_string(),
            interval: 30,
            digits: 6,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct TotpResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub totp: TotpResponseRaw,
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
    pub solved: Vec<RiddleAttempt>,
    pub rooms_entered: Vec<ObjectId>,
    pub jwt: Option<String>,
    pub totp: Option<TotpResponseRaw>,
    pub recovery_keys: Option<Vec<String>>,
    pub configured_2fa: Vec<SecondFactor>,
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
    pub ignore_case: bool,
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
    pub num_rooms: i32,
    pub num_riddles: i32,
    pub max_score: i32,
}

#[derive(Serialize, Debug)]
pub struct SecondFactorRequiredResponse {
    pub ok: bool,
    pub message: String,
    pub second_factors: Vec<SecondFactor>,
}

#[derive(Deserialize, Debug)]
pub struct WebAuthnRegisterStartRequest {
    pub username: String,
}

#[derive(Serialize, Debug)]
pub struct WebAuthnRegisterFinishResponse {
    pub ok: bool,
    pub message: Option<String>,
}

#[derive(Serialize, Debug)]
struct WebAuthnRegisterStartResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub ccr: CreationChallengeResponse,
}

#[derive(Serialize, Debug)]
struct WebAuthnLoginStartResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub rcr: RequestChallengeResponse,
}

#[derive(Serialize, Debug)]
struct WebAuthnLoginFinishResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub jwt: String,
}

#[derive(Serialize, Debug)]
struct MFARequiredResponse {
    pub ok: bool,
    pub message: Option<String>,
    #[serde(rename = "mfaMethods")]
    pub configured_2fa: Vec<SecondFactor>,
}

#[derive(Serialize, Debug)]
struct PromoteUserResponse {
    pub ok: bool,
    pub message: Option<String>,
    pub username: String,
    pub role: Role,
}

fn err_response(message: Option<String>) -> WithStatus<warp::reply::Json> {
    let reply = warp::reply::json(&json!(&StatusResponse {
        ok: false,
        message: message,
    }));
    warp::reply::with_status(reply, StatusCode::OK)
}

async fn get_room_by_id(room_id: &ObjectId, db: &DB) -> Result<RoomResponse> {
    let room_response = match db.get_room(room_id).await {
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
        Err(e) => return Err(e),
    };
    Ok(room_response)
}

pub async fn ping_handler() -> WebResult<impl Reply> {
    println!("ping_handler()");
    let reply: warp::reply::Json = warp::reply::json(&json!(&PingResponse {
        ok: true,
        message: Option::default(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn go_handler(direction_str: String, username: String, db: DB) -> WebResult<impl Reply> {
    println!(
        "go_handler(); direction = {}; username = {}",
        &direction_str, &username
    );
    let mut user: User = match db.get_user(&username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    let in_room = match &user.in_room {
        Some(in_room) => in_room,
        None => return Err(reject::custom(Error::UserIsInNoRoom)),
    };
    let room: Room = match db.get_room(&in_room).await {
        Ok(room) => {
            dbg!(room.id);
            room
        }
        Err(e) => return Err(reject::custom(e)),
    };
    let direction: &Direction = match room
        .neighbors
        .iter()
        .find(|&neighbor| neighbor.direction == direction_str)
    {
        Some(direction) => {
            dbg!(&direction_str, &direction.riddle_id);
            direction
        }
        None => return Err(reject::custom(Error::NeighborNotFoundError)),
    };
    let riddle_id: bson::oid::ObjectId = match user
        .solved
        .iter()
        .find(|&s| s.riddle_id == direction.riddle_id)
    {
        Some(riddle_attempt) => riddle_attempt.riddle_id,
        None => return Err(reject::custom(Error::RiddleNotSolvedError)),
    };
    let opposite: &String = &OPPOSITE[&direction.direction];
    let room_behind: Room = match db.get_room_behind(&opposite, &riddle_id).await {
        Ok(room_behind) => room_behind,
        Err(e) => return Err(reject::custom(e)),
    };
    println!(
        "moving from {} to {}",
        &user.in_room.unwrap(),
        &room_behind.id
    );
    user.in_room = Some(room_behind.id);
    // TODO: move all code accessing the database to db.rs
    let update_doc: bson::Document = match room.exit.is_some() && room.exit.unwrap() {
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
    let in_room = match &user.in_room {
        Some(in_room) => in_room,
        None => return Err(reject::custom(Error::UserIsInNoRoom)),
    };
    let room: Room = match db.get_room(&in_room).await {
        Ok(room) => {
            println!("new room {}", room.id);
            room
        }
        Err(e) => return Ok(err_response(Some(e.to_string()))),
    };
    let reply: warp::reply::Json = warp::reply::json(&json!(&SteppedThroughResponse {
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
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn riddle_solve_handler(
    riddle_id_str: OidString,
    body: RiddleSolveRequest,
    username: String,
    mut db: DB,
) -> WebResult<impl Reply> {
    let solution = url_escape::decode(&body.solution).into_owned();
    println!(
        "riddle_solve_handler(); riddle_id = {}, solution = {}",
        &riddle_id_str, &solution
    );
    let oid: bson::oid::ObjectId = match ObjectId::parse_str(riddle_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let (riddle_id, user, _msg) = db.riddle_accessibility(&oid, &username).await;
    let riddle_id = match riddle_id {
        Some(in_room) => in_room,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    let riddle: Option<Riddle> = match db.get_riddle_by_oid(&riddle_id).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle: Riddle = match riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    let solved: bool = match riddle.ignore_case.unwrap_or(false) {
        true => riddle.solution.to_lowercase() == solution.to_lowercase(),
        false => riddle.solution == solution,
    };
    let mut user: User = match user {
        Some(user) => user,
        None => return Err(reject::custom(Error::UserNotFoundError)),
    };
    if solved {
        let mut solutions: Vec<RiddleAttempt> = user.solved.clone();
        let riddle_attempt = match user.current_riddle_attempt {
            Some(ref riddle_attempt) => riddle_attempt,
            None => return Err(reject::custom(Error::RiddleHasNotBeenSeenByUser)),
        };
        if riddle_attempt.t0.is_none() {
            return Err(reject::custom(Error::RiddleHasNotBeenSeenByUser));
        }
        solutions.push(RiddleAttempt {
            riddle_id: riddle.id,
            t0: riddle_attempt.t0,
            t_solved: Some(Utc::now()),
        });
        user.level = riddle.level.max(user.level);
        user.score += riddle.difficulty;
        match db.set_user_solved(&solutions, &user).await {
            Ok(()) => {
                println!("User updated.");
            }
            Err(e) => {
                println!("Error: update failed: {}", &e);
                return Err(reject::custom(Error::RiddleNotSolvedError));
            }
        }
    } else {
        user.score -= riddle.deduction.unwrap_or(0);
        match db.rewrite_user_score(&user).await {
            Ok(()) => {
                println!("User updated.");
            }
            Err(e) => {
                println!("Error: update failed: {}", &e);
                return Err(reject::custom(Error::RiddleNotSolvedError));
            }
        }
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&RiddleSolvedResponse {
        ok: true,
        riddle_id: riddle.id,
        solved: solved,
        score: user.score,
        level: riddle.level,
        message: Option::default(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn debriefing_get_by_riddle_id_handler(
    riddle_id_str: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!(
        "debriefing_get_by_riddle_id_handler(); riddle_id = {}, username = {}",
        &riddle_id_str, &username
    );
    let oid: bson::oid::ObjectId = match ObjectId::parse_str(riddle_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let solved_riddle: Option<Riddle> = match db.get_riddle_if_solved(&oid, &username, None).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle: Riddle = match solved_riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    println!("got riddle {}", riddle.level);
    let reply: warp::reply::Json = warp::reply::json(&json!(&DebriefingResponse {
        ok: true,
        message: Option::default(),
        debriefing: riddle.debriefing,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn riddle_get_oid_handler(
    riddle_id_str: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("riddle_get_oid_handler(); riddle_id = {}", &riddle_id_str);
    let oid = match ObjectId::parse_str(riddle_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let (riddle_id, user, message) = db.riddle_accessibility(&oid, &username).await;
    let riddle_id: bson::oid::ObjectId = match riddle_id {
        Some(riddle_id) => riddle_id,
        None => return Ok(err_response(message)),
    };
    let riddle: Option<Riddle> = match db.get_riddle_by_oid(&riddle_id).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle: Riddle = match riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    let mut user = match user {
        Some(user) => user,
        None => return Err(reject::custom(Error::UserNotAssociatedWithRiddle)),
    };
    let riddle_attempt = RiddleAttempt {
        riddle_id,
        t0: Some(Utc::now()),
        t_solved: Option::default(),
    };
    user.current_riddle_attempt = Some(riddle_attempt);
    dbg!(&user.current_riddle_attempt);
    match db
        .get_users_coll()
        .update_one(
            doc! { "username": username.clone() },
            doc! {
                "$set": {
                    "current_riddle_attempt": Some(bson::to_bson(&riddle_attempt).unwrap()),
                },
            },
            None,
        )
        .await
    {
        Ok(_) => {
            println!("Updated current_riddle_attempt of user '{}'.", &username);
        }
        Err(e) => {
            println!("Error: update failed ({:?})", &e);
            return Err(reject::custom(Error::MongoQueryError(e)));
        }
    }

    println!("got riddle w/ level = {}", riddle.level);
    let mut found_files: Vec<FileResponse> = Vec::new();
    if let Some(files) = riddle.files {
        for file in files.iter() {
            println!("trying to load file {:?}", &file);
            let bucket: mongodb_gridfs::GridFSBucket =
                GridFSBucket::new(db.get_database(), Some(GridFSBucketOptions::default()));
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
    let reply: warp::reply::Json = warp::reply::json(&json!(&RiddleResponse {
        ok: true,
        message: Option::default(),
        id: riddle.id,
        level: riddle.level,
        difficulty: riddle.difficulty,
        deduction: riddle.deduction.unwrap_or(0),
        ignore_case: riddle.ignore_case.unwrap_or(false),
        files: Option::from(found_files),
        task: riddle.task,
        credits: riddle.credits,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn game_stats_handler(
    game_id_str: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!(
        "game_stats_handler(); game_id = {}, username = {}",
        &game_id_str, &username
    );
    let game_id: bson::oid::ObjectId = match ObjectId::parse_str(game_id_str) {
        Ok(oid) => oid,
        Err(e) => return Err(reject::custom(Error::BsonOidError(e))),
    };
    let num_rooms: Option<i32> = match db.get_num_rooms(&game_id).await {
        Ok(num_rooms) => num_rooms,
        Err(e) => return Err(reject::custom(e)),
    };
    let num_riddles: Option<i32> = match db.get_num_riddles(&game_id).await {
        Ok(num_riddles) => num_riddles,
        Err(e) => return Err(reject::custom(e)),
    };
    let max_score: Option<i32> = match db.get_max_score(&game_id).await {
        Ok(max_score) => max_score,
        Err(e) => return Err(reject::custom(e)),
    };
    let reply: warp::reply::Json = warp::reply::json(&json!(&GameStatsResponse {
        ok: true,
        message: Option::default(),
        num_rooms: num_rooms.unwrap_or(0),
        num_riddles: num_riddles.unwrap_or(0),
        max_score: max_score.unwrap_or(0),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn promote_user_handler(
    user_to_promote: String,
    role: String,
    username: String,
    mut db: DB,
) -> WebResult<impl Reply> {
    let user_to_promote = url_escape::decode(&user_to_promote).into_owned();
    let role = Role::from_str(&url_escape::decode(&role).into_owned());
    println!(
        "promote_user_handler() username = {}, user_to_promote = {}, role = {}",
        username, user_to_promote, role
    );
    if user_to_promote == username {
        return Err(reject::custom(Error::UserCannotChangeOwnRoleError));
    }
    let current_role = match db.get_user_role(&user_to_promote).await {
        Ok(role) => role,
        Err(e) => return Err(reject::custom(e)),
    };
    let user: User = match db.get_user(&username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    if role <= current_role {
        return Err(reject::custom(Error::CannotChangeToSameRole));
    }
    if user.role != Role::Admin {
        return Err(reject::custom(Error::UnsufficentRightsError));
    }
    match db.promote_user(&user_to_promote, &role).await {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    };
    let reply: warp::reply::Json = warp::reply::json(&json!(&PromoteUserResponse {
        ok: true,
        message: Option::default(),
        username: user_to_promote,
        role,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

// This function is needed for manual debugging.
pub async fn riddle_get_by_level_handler(
    level: u32,
    _username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("riddle_get_by_level_handler(); level = {}", level);
    let riddle: Option<Riddle> = match db.get_riddle_by_level(level).await {
        Ok(riddle) => riddle,
        Err(e) => return Err(reject::custom(e)),
    };
    let riddle: Riddle = match riddle {
        Some(riddle) => riddle,
        None => return Err(reject::custom(Error::RiddleNotFoundError)),
    };
    println!("got riddle w/ level = {}", riddle.level);
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
    let reply: warp::reply::Json = warp::reply::json(&json!(&RiddleResponse {
        ok: true,
        message: Option::default(),
        id: riddle.id,
        level: riddle.level,
        difficulty: riddle.difficulty,
        deduction: riddle.deduction.unwrap_or(0),
        ignore_case: riddle.ignore_case.unwrap_or(false),
        files: Option::from(found_files),
        task: riddle.task,
        credits: riddle.credits,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_authentication_handler(username: String) -> WebResult<impl Reply> {
    println!("user_authentication_handler(); username = {}", &username);
    Ok(StatusCode::OK)
}

pub async fn cheat_handler(username: String) -> WebResult<impl Reply> {
    println!("cheat_handler(); username = {}", username);
    if true {
        return Err(reject::custom(Error::CheatError));
    }
    Ok(StatusCode::PAYMENT_REQUIRED)
}

pub async fn user_whoami_handler(username: String, db: DB) -> WebResult<impl Reply> {
    println!("user_whoami_handler() {}", &username);
    let user: User = match db.get_user(&username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    println!("got user {} <{}>", &user.username, &user.email);
    let in_room: ObjectId = match user.in_room {
        Some(room) => room,
        None => return Err(reject::custom(Error::RoomNotFoundError)),
    };
    let room_response: RoomResponse = match get_room_by_id(&in_room, &db).await {
        Ok(room_response) => room_response,
        Err(e) => return Err(reject::custom(e)),
    };
    let mut configured_2fa: Vec<SecondFactor> = Vec::new();
    if user.totp_key.len() > 0 {
        configured_2fa.push(SecondFactor::Totp);
    }
    if user.webauthn.credentials.len() > 0 {
        configured_2fa.push(SecondFactor::Fido2);
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&UserWhoamiResponse {
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
        totp: Option::default(),
        recovery_keys: Option::default(),
        configured_2fa,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_totp_login_handler(body: UserTotpRequest, mut db: DB) -> WebResult<impl Reply> {
    println!(
        "user_totp_login_handler(); username = {}, totp = {}",
        &body.username, &body.totp
    );
    let user: User = match db.get_user(&body.username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    println!("got user {:?}", &user);
    if !user.awaiting_second_factor {
        return Err(reject::custom(Error::PointlessTotpError));
    }
    let mut configured_2fa: Vec<SecondFactor> = Vec::new();
    if user.webauthn.credentials.len() > 0 {
        configured_2fa.push(SecondFactor::Fido2);
    }
    if user.totp_key.len() > 0 {
        configured_2fa.push(SecondFactor::Totp);
        let seconds: u64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        match body.totp == totp_custom::<Sha1>(30, 6, &user.totp_key, seconds) {
            true => println!("TOTPs match"),
            false => {
                if body.totp == totp_custom::<Sha1>(30, 6, &user.totp_key, seconds - 30) {
                    println!("TOTPs match (after going back 30 secs)");
                } else {
                    return Err(reject::custom(Error::WrongCredentialsError));
                }
            }
        }
    }
    match db.login_user(&user).await {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    }
    let jwt: Option<String> = match auth::create_jwt(&user.username, &user.role) {
        Ok(jwt) => Some(jwt),
        Err(e) => return Err(reject::custom(e)),
    };
    let in_room: bson::oid::ObjectId = match user.in_room {
        Some(room) => room,
        None => return Err(reject::custom(Error::UserIsInNoRoom)),
    };
    // TODO: extract as function (see)
    let room_response: RoomResponse = match get_room_by_id(&in_room, &db).await {
        Ok(room_response) => room_response,
        Err(e) => return Err(reject::custom(e)),
    };
    let reply: warp::reply::Json = warp::reply::json(&json!(&UserWhoamiResponse {
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
        jwt,
        totp: Option::default(),
        recovery_keys: Option::default(),
        configured_2fa,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_login_handler(body: UserLoginRequest, mut db: DB) -> WebResult<impl Reply> {
    println!("user_login_handler(); username = {}", &body.username);
    let user: User = match db.get_user(&body.username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    println!("got user: {:?}", &user);
    let matches: bool = match Password::matches(&user.hash, &body.password) {
        Ok(matches) => matches,
        Err(_) => return Err(reject::custom(Error::HashingError)),
    };
    if !matches {
        return Err(reject::custom(Error::WrongCredentialsError));
    }
    println!("Hashes match.");
    let mut configured_2fa: Vec<SecondFactor> = Vec::new();
    let mut authenticated = true;
    if user.totp_key.len() > 0 {
        // if the TOTP is sent along the usual credentials, check if TOTP is correct
        if let Some(totp) = body.totp {
            let seconds: u64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            authenticated = match totp == totp_custom::<Sha1>(30, 6, &user.totp_key, seconds) {
                true => {
                    println!("TOTPs match");
                    true
                }
                false => return Err(reject::custom(Error::WrongCredentialsError)),
            }
        } else {
            authenticated = false;
            configured_2fa.push(SecondFactor::Totp);
            match db.set_user_awaiting_2fa(&user, true).await {
                Ok(()) => (),
                Err(e) => return Err(reject::custom(e)),
            }
        }
    }
    if user.webauthn.credentials.len() > 0 {
        authenticated = false;
        configured_2fa.push(SecondFactor::Fido2);
        match db.set_user_awaiting_2fa(&user, true).await {
            Ok(()) => (),
            Err(e) => return Err(reject::custom(e)),
        }
    }
    if authenticated {
        match db.login_user(&user).await {
            Ok(()) => (),
            Err(e) => return Err(reject::custom(e)),
        }
        let jwt: Option<String> = match auth::create_jwt(&user.username, &user.role) {
            Ok(jwt) => Some(jwt),
            Err(e) => return Err(reject::custom(e)),
        };
        let in_room: bson::oid::ObjectId = match user.in_room {
            Some(room) => room,
            None => return Err(reject::custom(Error::UserIsInNoRoom)),
        };
        let room_response: RoomResponse = match get_room_by_id(&in_room, &db).await {
            Ok(room_response) => room_response,
            Err(e) => return Err(reject::custom(e)),
        };
        let reply: warp::reply::Json = warp::reply::json(&json!(&UserWhoamiResponse {
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
            jwt,
            totp: Option::default(),
            recovery_keys: Option::default(),
            configured_2fa,
        }));
        Ok(warp::reply::with_status(reply, StatusCode::OK))
    } else {
        let reply: warp::reply::Json = warp::reply::json(&json!(&MFARequiredResponse {
            ok: false,
            message: Some("second factor required".to_string()),
            configured_2fa
        }));
        Ok(warp::reply::with_status(reply, StatusCode::OK))
    }
}

fn generate_otp_qrcode(username: &String, totp_key: &Vec<u8>) -> Result<(String, Vec<u8>)> {
    let b32_otp_secret: String =
        base32::encode(base32::Alphabet::RFC4648 { padding: false }, totp_key);
    let otp_str = format!(
        "otpauth://totp/{}: {}?secret={}&issuer={}",
        env!("CARGO_PKG_NAME"),
        username,
        b32_otp_secret,
        env!("CARGO_PKG_NAME"),
    );
    dbg!(&otp_str);
    let totp_qrcode: Vec<u8> =
        match qrcode_generator::to_png_to_vec(&otp_str, QrCodeEcc::Medium, 256) {
            Ok(code) => code,
            Err(_) => return Err(Error::TotpQrCodeGenerationError),
        };
    Ok((b32_otp_secret, totp_qrcode))
}

pub async fn user_totp_disable_handler(username: String, db: DB) -> WebResult<impl Reply> {
    println!("user_totp_disable_handler(); username = {}", &username);
    match db
        .get_users_coll()
        .update_one(
            doc! { "username": username.clone(), "activated": true },
            doc! {
                "$unset": {
                    "totp_key": 0,
                },
            },
            None,
        )
        .await
    {
        Ok(_) => {
            println!("Updated {}.", &username);
        }
        Err(e) => {
            println!("Error: update failed ({:?})", &e);
            return Err(reject::custom(Error::MongoQueryError(e)));
        }
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&StatusResponse {
        ok: true,
        message: Option::default(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_totp_enable_handler(username: String, db: DB) -> WebResult<impl Reply> {
    println!("user_totp_enable_handler(); username = {}", &username);
    let totp_key: Vec<u8> = rand::thread_rng().gen::<[u8; 32]>().to_vec();
    match db
        .get_users_coll()
        .update_one(
            doc! { "username": username.clone() },
            doc! {
                "$set": {
                    "totp_key": base64::encode(&totp_key),
                },
            },
            None,
        )
        .await
    {
        Ok(_) => {
            println!("Updated {}.", &username);
        }
        Err(e) => {
            println!("Error: update failed ({:?})", &e);
            return Err(reject::custom(Error::MongoQueryError(e)));
        }
    }
    let (secret, totp_qrcode) = match generate_otp_qrcode(&username, &totp_key) {
        Ok((secret, qrcode)) => (secret, qrcode),
        Err(e) => return Err(reject::custom(e)),
    };
    let reply: warp::reply::Json = warp::reply::json(&json!(&TotpResponse {
        ok: true,
        message: Option::default(),
        totp: TotpResponseRaw::new(totp_qrcode, secret),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_activation_handler(
    body: UserActivationRequest,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!(
        "user_activation_handler(); username = {}; pin = {}",
        &body.username, &body.pin
    );
    let mut user: User = match db.get_user_with_pin(&body.username, body.pin).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    match db.activate_user(&mut user).await {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    };
    let in_room: bson::oid::ObjectId = match user.in_room {
        Some(room) => room,
        None => return Err(reject::custom(Error::UserIsInNoRoom)),
    };
    let room_response: RoomResponse = match get_room_by_id(&in_room, &db).await {
        Ok(room_response) => room_response,
        Err(e) => return Err(reject::custom(e)),
    };
    let mut configured_2fa: Vec<SecondFactor> = Vec::new();
    let jwt: Option<String> = match auth::create_jwt(&user.username, &user.role) {
        Ok(jwt) => Some(jwt),
        Err(e) => return Err(reject::custom(e)),
    };
    let totp = match user.totp_key.is_empty() {
        true => Option::default(),
        false => {
            configured_2fa.push(SecondFactor::Totp);
            let (secret, totp_qrcode) = match generate_otp_qrcode(&user.username, &user.totp_key) {
                Ok((secret, qrcode)) => (secret, qrcode),
                Err(e) => return Err(reject::custom(e)),
            };
            Some(TotpResponseRaw::new(totp_qrcode, secret))
        }
    };
    let reply: warp::reply::Json = warp::reply::json(&json!(&UserWhoamiResponse {
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
        jwt,
        totp,
        recovery_keys: Some(user.recovery_keys),
        configured_2fa
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_password_change_handler(
    username: String,
    mut body: UserPasswordChangeRequest,
    mut db: DB,
) -> WebResult<impl Reply> {
    let password: String = body.password;
    body.password = "******".to_string();
    println!("user_password_change_handler(); body = {:?}", &body);
    if password.len() < 8 {
        return Err(reject::custom(Error::PasswordTooShortError));
    }
    let password_is_bad = match is_bad_password(&password) {
        Ok(bad) => bad,
        Err(_) => false, // soft fail
    };
    if password_is_bad {
        return Err(reject::custom(Error::UnsafePasswordError));
    }
    match db.set_user_password(&username, &password).await {
        Ok(()) => (),
        Err(e) => return Err(reject::custom(e)),
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&StatusResponse {
        ok: true,
        message: Option::default(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn user_registration_handler(
    mut body: UserRegistrationRequest,
    mut db: DB,
) -> WebResult<impl Reply> {
    let password: String = body.password;
    body.password = "******".to_string();
    println!("user_registration_handler(); body = {:?}", &body);
    if password.len() < 8 {
        return Err(reject::custom(Error::PasswordTooShortError));
    }
    let password_is_bad = match is_bad_password(&password) {
        Ok(bad) => bad,
        Err(_) => false, // soft fail
    };
    if password_is_bad {
        return Err(reject::custom(Error::UnsafePasswordError));
    }
    if !RE_USERNAME.is_match(&body.username.as_str()) {
        return Err(reject::custom(Error::InvalidUsernameError));
    }
    if !RE_MAIL.is_match(&body.email.as_str()) {
        return Err(reject::custom(Error::InvalidEmailError));
    }
    let taken = match db
        .is_username_or_email_taken(&body.username, &body.email)
        .await
    {
        Ok(taken) => taken,
        Err(e) => return Err(reject::custom(Error::DatabaseQueryError(e.to_string()))),
    };
    if taken {
        return Err(reject::custom(Error::UsernameOrEmailNotAvailableError));
    }
    let hash: String = match Password::hash(&password) {
        Ok(hash) => hash,
        Err(e) => return Err(reject::custom(e)),
    };
    let mut pin: PinType = 0;
    while pin == 0 {
        pin = OsRng.next_u32() % 1000000;
    }
    let totp_key: Vec<u8> = match body.second_factor {
        Some(SecondFactor::Totp) => rand::thread_rng().gen::<[u8; 32]>().to_vec(),
        _ => Vec::new(),
    };
    match db
        .create_user(&User::new(
            &body.username,
            &body.email,
            Role::User,
            hash,
            pin,
            totp_key,
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
    let email: lettre::Message = match Message::builder()
        .header(lettre::message::header::ContentType::TEXT_PLAIN)
        .from(
            "Labyrinth Mailer <nirwana@raetselonkel.de>"
                .parse()
                .unwrap(),
        )
        .to(to)
        .date_now()
        .subject("Deine Aktivierungs-PIN für Labyrinth")
        .body(format!(
            r#"Moin {}!

Du hast dich erfolgreich bei Labyrinth registriert.

Deine PIN zur Aktivierung des Accounts: {:06}

Bitte gib diese PIN auf der Labyrinth-Website ein.

Viele Grüße,
Dein Rätselonkel


*** Falls du keinen Schimmer hast, was es mit dieser Mail auf sich hat, kannst du sie getrost ignorieren ;-)"#,
            body.username, pin
        )) {
        Ok(email) => email,
        Err(_) => return Err(reject::custom(Error::MailBuilderError)), // TODO: propagate info of `lettre::error::Error`
    };
    let mailer: lettre::SmtpTransport = SmtpTransport::unencrypted_localhost();
    match mailer.send(&email) {
        Ok(_) => {
            println!(
                "Mail with PIN {:06} successfully sent to {} <{}>.",
                pin, body.username, body.email
            );
        }
        Err(_) => return Err(reject::custom(Error::SmtpTransportError)), // TODO: propagate info of `lettre::transport::smtp::Error`
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&StatusResponse {
        ok: true,
        message: Option::default(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::CREATED))
}

pub async fn webauthn_register_start_handler(
    username: String,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!(
        "webauthn_register_start_handler(); username = {}",
        &username
    );
    let wa_actor = webauthn::WebauthnActor::new(webauthn_default_config());
    let ccr = match wa_actor.challenge_register(&mut db, &username).await {
        Ok(ccr) => ccr,
        Err(_) => return Err(reject::custom(Error::WebauthnError)),
    };
    Ok(warp::reply::with_status(
        warp::reply::json(&json!(&WebAuthnRegisterStartResponse {
            ok: true,
            message: Option::default(),
            ccr: ccr,
        })),
        StatusCode::OK,
    ))
}

pub async fn webauthn_register_finish_handler(
    username: String,
    body: RegisterPublicKeyCredential,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!("webauthn_register_finish_handler(); body = {:?}", &body);
    let wa_actor = webauthn::WebauthnActor::new(webauthn_default_config());
    match wa_actor.register(&mut db, &username, &body).await {
        Ok(()) => (),
        Err(_) => return Err(reject::custom(Error::WebauthnError)),
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&WebAuthnRegisterFinishResponse {
        ok: true,
        message: Option::default(),
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn webauthn_login_start_handler(username: String, mut db: DB) -> WebResult<impl Reply> {
    println!("webauthn_login_start_handler(); username = {}", &username);
    let wa_actor = webauthn::WebauthnActor::new(webauthn_default_config());
    let rcr = match wa_actor.challenge_authenticate(&mut db, &username).await {
        Ok(rcr) => rcr,
        Err(_) => return Err(reject::custom(Error::WebauthnError)),
    };
    Ok(warp::reply::with_status(
        warp::reply::json(&json!(&WebAuthnLoginStartResponse {
            ok: true,
            message: Option::default(),
            rcr: rcr,
        })),
        StatusCode::OK,
    ))
}

pub async fn webauthn_login_finish_handler(
    username: String,
    body: PublicKeyCredential,
    mut db: DB,
) -> WebResult<impl Reply> {
    println!(
        "webauthn_login_finish_handler(); username = {}, body = {:?}",
        &username, &body
    );
    let user: User = match db.get_user(&username).await {
        Ok(user) => user,
        Err(e) => return Err(reject::custom(e)),
    };
    let wa_actor = webauthn::WebauthnActor::new(webauthn_default_config());
    match wa_actor.authenticate(&mut db, &user, &body).await {
        Ok(()) => (),
        Err(_) => return Err(reject::custom(Error::WebauthnError)),
    }
    match db.set_user_awaiting_2fa(&user, false).await {
        Ok(()) => (),
        Err(_) => return Err(reject::custom(Error::WebauthnError)),
    }
    let jwt: Option<String> = match auth::create_jwt(&username, &user.role) {
        Ok(jwt) => Some(jwt),
        Err(e) => return Err(reject::custom(e)),
    };
    let in_room: ObjectId = match user.in_room {
        Some(room) => room,
        None => return Err(reject::custom(Error::UserIsInNoRoom)),
    };
    let room_response: RoomResponse = match get_room_by_id(&in_room, &db).await {
        Ok(room_response) => room_response,
        Err(e) => return Err(reject::custom(e)),
    };
    let mut configured_2fa: Vec<SecondFactor> = Vec::new();
    if user.totp_key.len() > 0 {
        configured_2fa.push(SecondFactor::Totp);
    }
    if user.webauthn.credentials.len() > 0 {
        configured_2fa.push(SecondFactor::Fido2);
    }
    let reply: warp::reply::Json = warp::reply::json(&json!(&UserWhoamiResponse {
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
        jwt,
        totp: Option::default(),
        recovery_keys: Option::default(),
        configured_2fa,
    }));
    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
    const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    println!("{} {}", CARGO_PKG_NAME, CARGO_PKG_VERSION);
    let db = DB::init().await?;
    let root = warp::path::end().map(|| "Labyrinth API root.");
    /* Routes accessible to all users */
    let ping_route = warp::path!("ping").and(warp::get()).and_then(ping_handler);
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
    let user_password_route = warp::path!("user" / "passwd")
        .and(warp::post())
        .and(with_auth(Role::User))
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_password_change_handler);
    let user_totp_login_route = warp::path!("user" / "totp" / "login")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_totp_login_handler);
    let user_totp_enable_route = warp::path!("user" / "totp" / "enable")
        .and(warp::post())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(user_totp_enable_handler);
    let user_totp_disable_route = warp::path!("user" / "totp" / "disable")
        .and(warp::post())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(user_totp_disable_handler);
    let webauthn_login_start_route = warp::path!("user" / "webauthn" / "login" / "start" / String)
        .and(warp::post())
        .and(with_db(db.clone()))
        .and_then(webauthn_login_start_handler);
    let webauthn_login_finish_route =
        warp::path!("user" / "webauthn" / "login" / "finish" / String)
            .and(warp::post())
            .and(warp::body::json())
            .and(with_db(db.clone()))
            .and_then(webauthn_login_finish_handler);
    /* Routes accessible only to authorized users */
    let webauthn_register_start_route = warp::path!("user" / "webauthn" / "register" / "start")
        .and(warp::post())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(webauthn_register_start_handler);
    let webauthn_register_finish_route = warp::path!("user" / "webauthn" / "register" / "finish")
        .and(warp::post())
        .and(with_auth(Role::User))
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(webauthn_register_finish_handler);
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
    let debriefing_get_by_riddle_id_route = warp::path!("riddle" / "debriefing" / OidString)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(debriefing_get_by_riddle_id_handler);
    let riddle_solve_route = warp::path!("riddle" / "solve" / OidString)
        .and(warp::post())
        .and(warp::body::json())
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
    let riddle_get_by_level_route = warp::path!("admin" / "riddle" / "by" / "level" / u32)
        .and(warp::get())
        .and(with_auth(Role::Admin))
        .and(with_db(db.clone()))
        .and_then(riddle_get_by_level_handler);
    let promote_user_route = warp::path!("admin" / "promote" / String / String)
        .and(warp::get())
        .and(with_auth(Role::Admin))
        .and(with_db(db.clone()))
        .and_then(promote_user_handler);

    let routes = root
        .or(riddle_get_by_oid_route)
        .or(debriefing_get_by_riddle_id_route)
        .or(riddle_get_by_level_route)
        .or(promote_user_route)
        .or(riddle_solve_route)
        .or(go_route)
        .or(user_whoami_route)
        .or(user_auth_route)
        .or(user_login_route)
        .or(user_password_route)
        .or(user_totp_enable_route)
        .or(user_totp_disable_route)
        .or(user_totp_login_route)
        .or(user_register_route)
        .or(user_activation_route)
        .or(webauthn_register_start_route)
        .or(webauthn_register_finish_route)
        .or(webauthn_login_start_route)
        .or(webauthn_login_finish_route)
        .or(ping_route)
        .or(cheat_route)
        .or(game_stats_route)
        .or(warp::any().and(warp::options()).map(warp::reply))
        .recover(error::handle_rejection);

    let host = env::var("API_HOST").expect("API_HOST is not in .env file");
    let addr: SocketAddr = host.parse().expect("Cannot parse host address");
    println!("Listening on http://{}", host);
    warp::serve(routes).run(addr).await;
    Ok(())
}
