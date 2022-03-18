use auth::{with_auth, Role};
use chrono::{serde::ts_seconds_option, DateTime, Utc};
use db::{with_db, Password, PinType, User, DB};
use lettre::{Message, SmtpTransport, Transport};
use pbkdf2::{
    password_hash::{Ident, PasswordHasher, SaltString},
    Algorithm, Params, Pbkdf2,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::From;
use std::net::SocketAddr;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::{http::StatusCode, Filter, Rejection, Reply};

mod auth;
mod db;
mod error;

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
            }));
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            return Ok(reply);
        }
        Err(_) => {
            let empty: Vec<u8> = Vec::new();
            let reply = warp::reply::json(&empty);
            let reply = warp::reply::with_status(reply, StatusCode::UNAUTHORIZED);
            return Ok(reply);
        }
    }
}

pub async fn null_handler() -> WebResult<impl Reply> {
    println!("null called",);
    Ok(StatusCode::OK)
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
            let empty: Vec<u8> = Vec::new();
            let reply = warp::reply::json(&empty);
            let reply = warp::reply::with_status(reply, StatusCode::UNAUTHORIZED);
            return Ok(reply);
        }
    }
    let empty: Vec<u8> = Vec::new();
    let reply = warp::reply::json(&empty);
    let reply = warp::reply::with_status(reply, StatusCode::UNAUTHORIZED);
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
            let empty: Vec<u8> = Vec::new();
            let reply = warp::reply::json(&empty);
            let reply = warp::reply::with_status(reply, StatusCode::FORBIDDEN);
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
            let empty: Vec<u8> = Vec::new();
            let reply = warp::reply::json(&empty);
            let reply = warp::reply::with_status(reply, StatusCode::FORBIDDEN);
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
            let empty: Vec<u8> = Vec::new();
            let reply = warp::reply::json(&empty);
            let reply = warp::reply::with_status(reply, StatusCode::OK);
            Ok(reply)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let db = DB::init().await?;
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
    let root = warp::path::end().map(|| "Labyrinth API root.");
    let ping_route = warp::path!("ping")
        .and(warp::get())
        .map(warp::reply)
        .with(warp::reply::with::headers(headers.clone()));
    let user_register_route = warp::path!("user" / "register")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_registration_handler)
        .with(warp::reply::with::headers(headers.clone()));
    let cors_user_register_route = warp::path!("user" / "register")
        .and(warp::options().and_then(null_handler))
        .with(warp::reply::with::headers(headers.clone()));
    let user_activation_route = warp::path!("user" / "activate")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_activation_handler)
        .with(warp::reply::with::headers(headers.clone()));
    let cors_user_activation_route = warp::path!("user" / "activate")
        .and(warp::options().and_then(null_handler))
        .with(warp::reply::with::headers(headers.clone()));
    let user_login_route = warp::path!("user" / "login")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(user_login_handler)
        .with(warp::reply::with::headers(headers.clone()));
    let cors_user_login_route = warp::path!("user" / "login")
        .and(warp::options().and_then(null_handler))
        .with(warp::reply::with::headers(headers.clone()));
    let user_auth_route = warp::path!("user" / "auth")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and_then(user_authentication_handler)
        .with(warp::reply::with::headers(headers.clone()));
    let cors_auth_route = warp::path!("user" / "auth")
        .and(warp::options())
        .and_then(null_handler)
        .with(warp::reply::with::headers(headers.clone()));
    let user_whoami_route = warp::path!("user" / "whoami")
        .and(warp::get())
        .and(with_auth(Role::User))
        .and(with_db(db.clone()))
        .and_then(user_whoami_handler)
        .with(warp::reply::with::headers(headers.clone()));
    let cors_whoami_route = warp::path!("user" / "whoami")
        .and(warp::options())
        .and_then(null_handler)
        .with(warp::reply::with::headers(headers.clone()));

    let routes = root
        .or(user_whoami_route)
        .or(cors_whoami_route)
        .or(user_auth_route)
        .or(cors_user_login_route)
        .or(user_login_route)
        .or(user_register_route)
        .or(cors_user_register_route)
        .or(user_activation_route)
        .or(cors_user_activation_route)
        .or(ping_route)
        .or(cors_auth_route)
        // TODO: Add CORS headers to rejection response
        .recover(error::handle_rejection);

    let host = "127.0.0.1:8181";
    let addr: SocketAddr = host.parse().expect("Cannot parse host address");
    println!("Listening on http://{}", host);
    warp::serve(routes).run(addr).await;
    Ok(())
}
