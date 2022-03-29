/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::convert::From;
use std::env;
use std::io::Read;
use std::net::SocketAddr;
use url_escape;
use warp::{http::StatusCode, Filter, Rejection, Reply};

mod auth;
mod b64;
mod db;
mod error;

type Result<T> = std::result::Result<T, error::Error>;
type WebResult<T> = std::result::Result<T, Rejection>;

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
    pub id: ObjectId,
    pub number: u32,
    pub neighbors: Vec<Direction>,
    pub game_id: ObjectId,
    pub entry: Option<bool>,
    pub exit: Option<bool>,
}

#[derive(Serialize, Debug)]
pub struct UserWhoamiResponse {
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
    pub in_room: Option<RoomResponse>,
    pub solved: Vec<ObjectId>,
    pub rooms_entered: Vec<ObjectId>,
    pub jwt: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FileVariantResponse {
    pub name: String,
    #[serde(with = "b64")]
    pub data: Vec<u8>,
    #[serde(rename = "mimeType")]
    pub scale: Option<u32>,
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
    pub scale: Option<u32>,
    pub variants: Option<Vec<FileVariantResponse>>,
}

#[derive(Serialize, Debug)]
pub struct RiddleResponse {
    pub id: ObjectId,
    pub level: u32,
    pub files: Option<Vec<FileResponse>>,
    pub task: Option<String>,
    pub difficulty: u32,
    pub credits: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct RiddleSolvedResponse {
    pub riddle_id: ObjectId,
    pub solved: bool,
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
    pub num_rooms: u32,
    pub num_riddles: u32,
    pub max_score: u32,
}

pub async fn go_handler(direction: String, username: String, db: DB) -> WebResult<impl Reply> {
    println!(
        "go_handler called, direction = {}, username = {}",
        direction, username
    );
    match db.get_user(&username).await {
        Ok(ref mut user) => match db.get_room(&user.in_room.unwrap()).await {
            Ok(room) => {
                println!("current room: {}", room.id);
                match room
                    .neighbors
                    .iter()
                    .find(|&neighbor| neighbor.direction == direction)
                {
                    Some(neighbor) => {
                        println!("riddle in direction {}: {}", direction, neighbor.riddle_id);
                        match user.solved.iter().find(|&&s| s == neighbor.riddle_id) {
                            Some(riddle_id) => {
                                let opposite = &OPPOSITE[&neighbor.direction];
                                match db
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
                                    Ok(room_behind) => {
                                        println!(
                                            "Room in opposite direction {}: {}",
                                            opposite,
                                            room_behind.as_ref().unwrap().id
                                        );
                                        user.in_room = Some(room_behind.unwrap().id);
                                        match db
                                            .get_users_coll()
                                            .update_one(
                                                doc! { "_id": user.id, "activated": true },
                                                doc! {
                                                    "$set": { "in_room": user.in_room },
                                                    "$addToSet": { "rooms_entered": user.in_room },
                                                },
                                                None,
                                            )
                                            .await
                                        {
                                            Ok(_) => {
                                                match db.get_room(&user.in_room.unwrap()).await {
                                                    Ok(room) => {
                                                        let reply = warp::reply::json(&json!(
                                                            &SteppedThroughResponse {
                                                                ok: true,
                                                                message: Option::default(),
                                                                room: RoomResponse {
                                                                    id: room.id,
                                                                    number: room.number,
                                                                    entry: room.entry,
                                                                    exit: room.exit,
                                                                    game_id: room.game_id,
                                                                    neighbors: room.neighbors,
                                                                },
                                                            }
                                                        ));
                                                        let reply = warp::reply::with_status(
                                                            reply,
                                                            StatusCode::OK,
                                                        );
                                                        Ok(reply)
                                                    }
                                                    Err(e) => {
                                                        let reply = warp::reply::json(&json!(
                                                            &StatusResponse {
                                                                ok: false,
                                                                message: Option::from(format!(
                                                            "Error: room behind not found ({:?}",
                                                            e
                                                        )),
                                                            }
                                                        ));
                                                        let reply = warp::reply::with_status(
                                                            reply,
                                                            StatusCode::OK,
                                                        );
                                                        Ok(reply)
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let reply =
                                                    warp::reply::json(&json!(&StatusResponse {
                                                        ok: false,
                                                        message: Option::from(format!(
                                                            "Error: update failed ({:?}",
                                                            e
                                                        )),
                                                    }));
                                                let reply =
                                                    warp::reply::with_status(reply, StatusCode::OK);
                                                Ok(reply)
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let reply = warp::reply::json(&json!(&StatusResponse {
                                            ok: false,
                                            message: Option::from(format!(
                                                "Error: no room behind found ({:?})",
                                                e
                                            )),
                                        }));
                                        let reply = warp::reply::with_status(reply, StatusCode::OK);
                                        Ok(reply)
                                    }
                                }
                            }
                            None => {
                                let reply = warp::reply::json(&json!(&StatusResponse {
                                    ok: false,
                                    message: Option::from("riddle not solved".to_string()),
                                }));
                                let reply = warp::reply::with_status(reply, StatusCode::OK);
                                Ok(reply)
                            }
                        }
                    }
                    None => {
                        let reply = warp::reply::json(&json!(&StatusResponse {
                            ok: false,
                            message: Option::from("neighbor not found".to_string()),
                        }));
                        let reply = warp::reply::with_status(reply, StatusCode::OK);
                        Ok(reply)
                    }
                }
            }
            Err(_) => {
                let reply = warp::reply::json(&json!(&StatusResponse {
                    ok: false,
                    message: Option::from("room not found".to_string()),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                Ok(reply)
            }
        },
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from("user not found".to_string()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
}

pub async fn riddle_solve_handler(
    riddle_id_str: String,
    solution: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    let solution = url_escape::decode(&solution);
    println!(
        "riddle_solve_handler called, oid = {}, solution = {}",
        riddle_id_str, solution
    );
    let oid = ObjectId::parse_str(&riddle_id_str).unwrap();
    let (riddle_id, user, reply) = db.is_riddle_accessible(&oid, &username).await;
    match riddle_id {
        Some(riddle_id) => match db.get_riddle_by_oid(riddle_id).await {
            Ok(riddle) => match riddle {
                Some(riddle) => {
                    println!(
                        "got riddle {}, difficulty {}",
                        riddle.level, riddle.difficulty
                    );
                    let solved = riddle.solution == solution;
                    let mut user = user.unwrap();
                    if solved {
                        let mut solutions = user.solved;
                        solutions.push(riddle.id);
                        user.level = riddle.level.max(user.level);
                        user.score += riddle.difficulty;
                        let result = db
                            .get_users_coll()
                            .update_one(
                                doc! { "_id": user.id, "activated": true },
                                doc! {
                                    "$set": { "solved": solutions, "level": user.level, "score": user.score },
                                },
                                None,
                            )
                            .await;
                        match result {
                            Ok(_) => {
                                println!("User updated.");
                            }
                            Err(e) => {
                                println!("Error: update failed ({:?})", e);
                            }
                        }
                    }
                    let reply = warp::reply::json(&json!(&RiddleSolvedResponse {
                        riddle_id: riddle.id,
                        solved: solved,
                        level: riddle.level,
                        message: Option::default(),
                    }));
                    let reply = warp::reply::with_status(reply, StatusCode::OK);
                    Ok(reply)
                }
                None => {
                    let reply = warp::reply::json(&json!(&StatusResponse {
                        ok: false,
                        message: Option::from("riddle not found".to_string()),
                    }));
                    let reply = warp::reply::with_status(reply, StatusCode::OK);
                    Ok(reply)
                }
            },
            Err(_) => {
                let reply = warp::reply::json(&json!(&StatusResponse {
                    ok: false,
                    message: Option::from("either username or password is wrong".to_string()),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                Ok(reply)
            }
        },
        None => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: reply,
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
}

pub async fn riddle_get_oid_handler(
    riddle_id_str: String,
    username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("riddle_get_oid_handler called, oid = {}", riddle_id_str);
    let oid = ObjectId::parse_str(riddle_id_str).unwrap();
    let (riddle_id, _user, reply) = db.is_riddle_accessible(&oid, &username).await;
    match riddle_id {
        Some(riddle_id) => match db.get_riddle_by_oid(riddle_id).await {
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
                            let mut cursor =
                                bucket.open_download_stream(file.file_id).await.unwrap();
                            let mut data: Vec<u8> = Vec::new();
                            while let Some(mut chunk) = cursor.next().await {
                                data.append(&mut chunk);
                            }
                            found_files.push(FileResponse {
                                id: file.file_id,
                                name: file.name.clone(),
                                data: data,
                                mime_type: file.mime_type.clone(),
                                scale: file.scale,
                                width: file.width,
                                height: file.height,
                                variants: Option::default(),
                            })
                        }
                    }
                    let reply = warp::reply::json(&json!(&RiddleResponse {
                        id: riddle.id,
                        level: riddle.level,
                        difficulty: riddle.difficulty,
                        files: Option::from(found_files),
                        task: Option::from(riddle.task),
                        credits: Option::from(riddle.credits),
                    }));
                    let reply = warp::reply::with_status(reply, StatusCode::OK);
                    Ok(reply)
                }
                None => {
                    let reply = warp::reply::json(&json!(&StatusResponse {
                        ok: false,
                        message: Option::from("riddle not found".to_string()),
                    }));
                    let reply = warp::reply::with_status(reply, StatusCode::OK);
                    Ok(reply)
                }
            },
            Err(_) => {
                let reply = warp::reply::json(&json!(&StatusResponse {
                    ok: false,
                    message: Option::from("either username or password is wrong".to_string()),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                Ok(reply)
            }
        },
        None => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: reply,
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
}

pub async fn game_stats_handler(
    game_id: String,
    _username: String,
    db: DB,
) -> WebResult<impl Reply> {
    println!("game_stats_handler called, game_id = {}", game_id);
    let game_id = ObjectId::parse_str(game_id).unwrap();
    let num_rooms = match db.get_num_rooms(&game_id).await {
        Ok(num_rooms) => num_rooms,
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from(format!("no rooms found for game ID {}", game_id)),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            return Ok(reply);
        }
    };
    let num_riddles = match db.get_num_riddles(&game_id).await {
        Ok(num_riddles) => num_riddles,
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from(format!("no riddles found for game ID {}", game_id)),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            return Ok(reply);
        }
    };
    let reply = warp::reply::json(&json!(&GameStatsResponse {
        ok: true,
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
                            scale: file.scale,
                            width: file.width,
                            height: file.height,
                            variants: Option::default(),
                        })
                    }
                }
                let reply = warp::reply::json(&json!(&RiddleResponse {
                    id: riddle.id,
                    level: riddle.level,
                    difficulty: riddle.difficulty,
                    files: Option::from(found_files),
                    task: Option::from(riddle.task),
                    credits: Option::from(riddle.credits),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                Ok(reply)
            }
            None => {
                let reply = warp::reply::json(&json!(&StatusResponse {
                    ok: false,
                    message: Option::from("riddle not found".to_string()),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                Ok(reply)
            }
        },
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: true,
                message: Option::from("either username or password is wrong".to_string()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
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
            let room_response = match db.get_room(&user.in_room.unwrap()).await {
                Ok(room) => Option::from(RoomResponse {
                    id: room.id,
                    number: room.number,
                    neighbors: room.neighbors,
                    game_id: room.game_id,
                    entry: room.entry,
                    exit: room.exit,
                }),
                Err(_) => None,
            };
            let reply = warp::reply::json(&json!(&UserWhoamiResponse {
                username: user.username.clone(),
                email: user.email.clone(),
                role: user.role.clone(),
                activated: user.activated,
                created: Option::from(user.created),
                registered: Option::from(user.registered),
                last_login: Option::from(user.last_login),
                level: user.level,
                score: user.score,
                in_room: room_response,
                solved: user.solved,
                rooms_entered: user.rooms_entered,
                jwt: Option::default(),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from("user not found".to_string()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
}

pub async fn user_login_handler(body: UserLoginRequest, mut db: DB) -> WebResult<impl Reply> {
    println!("user_login_handler called, username = {}", body.username);
    match db.get_user(&body.username).await {
        Ok(user) => {
            println!("got user {}\nwith hash  = {}", user.username, user.hash);
            let matches = argon2::verify_encoded(&user.hash, body.password.as_bytes()).unwrap();
            if matches {
                println!("Hashes match. User is verified.");
                let _ = db.login_user(&user).await;
                let token_str = auth::create_jwt(&user.username, &user.role);
                let room_response = match db.get_room(&user.in_room.unwrap()).await {
                    Ok(room) => Option::from(RoomResponse {
                        id: room.id,
                        number: room.number,
                        neighbors: room.neighbors,
                        game_id: room.game_id,
                        entry: room.entry,
                        exit: room.exit,
                    }),
                    Err(_) => None,
                };
                let reply = warp::reply::json(&json!(&UserWhoamiResponse {
                    username: user.username.clone(),
                    email: user.email.clone(),
                    role: user.role.clone(),
                    activated: user.activated,
                    created: Option::from(user.created),
                    registered: Option::from(user.registered),
                    last_login: Option::from(user.last_login),
                    level: user.level,
                    score: user.score,
                    in_room: room_response,
                    solved: user.solved,
                    rooms_entered: user.rooms_entered,
                    jwt: Option::from(token_str.unwrap()),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                return Ok(reply);
            } else {
                let reply = warp::reply::json(&json!(&StatusResponse {
                    ok: false,
                    message: Option::from("either username or password is wrong".to_string()),
                }));
                let reply = warp::reply::with_status(reply, StatusCode::OK);
                Ok(reply)
            }
        }
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from("user not found".to_string()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
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
        Ok(mut user) => {
            let _ = db.activate_user(&mut user).await;
            let token_str = auth::create_jwt(&user.username, &user.role);
            let room_response = match db.get_room(&user.in_room.unwrap()).await {
                Ok(room) => Option::from(RoomResponse {
                    id: room.id,
                    number: room.number,
                    neighbors: room.neighbors,
                    game_id: room.game_id,
                    entry: room.entry,
                    exit: room.exit,
                }),
                Err(_) => None,
            };
            let reply = warp::reply::json(&json!(&UserWhoamiResponse {
                username: user.username.clone(),
                email: user.email.clone(),
                role: user.role.clone(),
                activated: user.activated,
                created: Option::from(user.created),
                registered: Option::from(user.registered),
                last_login: Option::from(user.last_login),
                level: user.level,
                score: user.score,
                in_room: room_response,
                solved: user.solved,
                rooms_entered: user.rooms_entered,
                jwt: Option::from(token_str.unwrap()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
        Err(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from("User/PIN not found".to_string()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
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
    if body.password.len() < 8 {
        let reply = warp::reply::json(&json!(&StatusResponse {
            ok: false,
            message: Option::from("password must be at least 8 characters long".to_string()),
        }));
        let reply = warp::reply::with_status(reply, StatusCode::OK);
        return Ok(reply);
    }
    if bad_password(&body.password) {
        println!("BAD PASSWORD: {}", body.password);
        let reply = warp::reply::json(&json!(&StatusResponse {
            ok: false,
            message: Option::from("unsafe password".to_string()),
        }));
        let reply = warp::reply::with_status(reply, StatusCode::OK);
        return Ok(reply);
    }
    let user = db.get_user(&body.username).await;
    match user {
        Ok(_) => {
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: false,
                message: Option::from("username not available".to_string()),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
        Err(_) => {
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
            let salt: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
            let hash = argon2::hash_encoded(body.password.as_bytes(), &salt, &config).unwrap();
            let mut pin: PinType = 0;
            while pin == 0 {
                pin = OsRng.next_u32() % 1000000;
            }
            let _result = db
                .create_user(&User::new(
                    &body.username,
                    &body.email,
                    body.role,
                    hash,
                    Option::from(pin),
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
            let reply = warp::reply::json(&json!(&StatusResponse {
                ok: true,
                message: Option::default(),
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
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
    let riddle_get_by_oid_route = warp::path!("riddle" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(riddle_get_oid_handler);
    let riddle_solve_route = warp::path!("riddle" / "solve" / String / "with" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(riddle_solve_handler);
    let go_route = warp::path!("go" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(go_handler);
    let game_stats_route = warp::path!("game" / "stats" / String)
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(game_stats_handler);
    /* Routes accessible only to authorized admins */
    let riddle_get_by_level_route = warp::path!("riddle" / "by" / "level" / u32)
        .and(warp::get())
        .and(with_auth(Role::Admin))
        .and(with_db(db.clone()))
        .and_then(riddle_get_by_level_handler);

    let routes = root
        .or(riddle_get_by_oid_route)
        .or(riddle_get_by_level_route)
        .or(riddle_solve_route)
        .or(go_route)
        .or(user_whoami_route)
        .or(user_auth_route)
        .or(user_login_route)
        .or(user_register_route)
        .or(user_activation_route)
        .or(ping_route)
        .or(game_stats_route)
        .or(warp::any().and(warp::options()).map(warp::reply));
    //.recover(error::handle_rejection);

    const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
    const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    println!("{} {}", CARGO_PKG_NAME, CARGO_PKG_VERSION);
    let host = env::var("API_HOST").expect("API_HOST is not in .env file");
    let addr: SocketAddr = host.parse().expect("Cannot parse host address");
    println!("Listening on http://{}", host);
    warp::serve(routes).run(addr).await;
    Ok(())
}
