use auth::{with_auth, Role};
use bson::Bson;
use error::Error::*;
// use futures::TryStreamExt;
use lettre::{Message, SmtpTransport, Transport};
use mongodb::bson::doc;
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection, Database};
use pbkdf2::{
    password_hash::{Ident, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Params, Pbkdf2,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::From;
use std::convert::Infallible;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::{http::StatusCode, Filter, Rejection, Reply};

mod auth;
mod error;

type Result<T> = std::result::Result<T, error::Error>;
type WebResult<T> = std::result::Result<T, Rejection>;
type PinType = u32;

const DB_NAME: &str = "labyrinth";
const USERS_COLL: &str = "users";
// const ROOMS_COLL: &str = "rooms";

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
}

impl User {
    pub fn new(
        username: &String,
        email: &String,
        role: &String,
        password: Option<Password>,
        pin: Option<PinType>,
        activated: bool,
    ) -> Self {
        User {
            username: username.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            password: password,
            pin: pin,
            activated: activated,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct UserActivationResponse {
    pub jwt: String,
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

    pub async fn activate_user(&mut self, user: &User) -> Result<()> {
        let query = doc! { "username": user.username.clone(), "activated": false };
        let modification = doc! {
            "$set": { "activated": true},
            "$unset": { "pin": 0 }
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

fn with_db(db: DB) -> impl Filter<Extract = (DB,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

pub async fn user_authentication_handler(username: String) -> WebResult<impl Reply> {
    println!(
        "user_authentication_handler called, username = {}",
        username
    );
    Ok(StatusCode::OK)
}

pub async fn user_login_handler(body: UserLoginRequest, db: DB) -> WebResult<impl Reply> {
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
            if password_hash == user.password.unwrap().hash {
                println!("Hashes match. User is verified.");
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
                .body(format!("Hi {}!\n\nYou've successfully registered with Labyrinth.\n\nYour PIN: {:06}\n\nPlease head back to the Labyrinth website and enter it to activate your account.\n\nCheers,\nYour Labyrinth Host\n\n\n*** If you don't what this mail is about, please ignore it ;-)", body.username, pin))
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
    headers.insert("Access-Control-Allow-Headers", HeaderValue::from_static("x-csrf-token,authorization,content-type,accept,origin,x-requested-with,access-control-allow-origin"));
    headers.insert("Allow-Credentials", HeaderValue::from_static("true"));
    headers.insert(
        "Allow-Methods",
        HeaderValue::from_static("GET,POST,PUT,PATCH,OPTIONS,DELETE"),
    );
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
    let root = warp::path::end().map(|| "Labyrinth API root.");

    // TODO: implement OPTIONS reply for individual paths, not for any
    let cors_route = warp::any()
        .map(warp::reply)
        .with(warp::reply::with::headers(headers.clone()));
    let routes = root
        .or(user_auth_route.with(warp::reply::with::headers(headers.clone())))
        .or(user_login_route.with(warp::reply::with::headers(headers.clone())))
        .or(user_register_route.with(warp::reply::with::headers(headers.clone())))
        .or(user_activation_route.with(warp::reply::with::headers(headers.clone())))
        .or(ping_route.with(warp::reply::with::headers(headers.clone())))
        .or(cors_route);

    let client_options = ClientOptions::parse("mongodb://localhost:27017")
        .await
        .unwrap();
    let client = Client::with_options(client_options).unwrap();
    println!("Databases:");
    for db_name in client.list_database_names(None, None).await {
        for x in db_name {
            println!(" - {}", x);
        }
    }
    println!("Collections in 'labyrinth':");
    let db = client.database("labyrinth");
    for collection_name in db.list_collection_names(None).await.unwrap() {
        println!(" - {}", collection_name);
    }

    println!("Listening on http://127.0.0.1:8181");
    warp::serve(routes).run(([127, 0, 0, 1], 8181)).await;
    Ok(())
}
