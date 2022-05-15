/**
 * Copyright (c) 2022 Oliver Lau <oliver@ersatzworld.net>
 * All rights reserved.
 */
use crate::{error::Error, Result, WebResult};
use chrono::prelude::*;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use lazy_static::lazy_static;
use log;
use serde::{Deserialize, Serialize};
use std::fmt;
use warp::{
    filters::header::headers_cloned,
    http::header::{HeaderMap, HeaderValue, AUTHORIZATION},
    reject, Filter, Rejection,
};

const BEARER: &str = "Bearer ";

pub struct JwtSecretKey {
    pub token: Vec<u8>,
}

impl JwtSecretKey {
    pub fn new() -> JwtSecretKey {
        JwtSecretKey { token: Vec::new() }
    }
    pub fn new_from_file(path: &str) -> JwtSecretKey {
        let mut jwt: JwtSecretKey = JwtSecretKey::new();
        jwt.read_key(path);
        jwt
    }
    fn read_key(&mut self, path: &str) {
        log::info!("Reading JWT_SECRET_KEY ...");
        match std::fs::read(path) {
            Ok(bytes) => {
                self.token = bytes;
            }
            Err(e) => {
                panic!("{}", e);
            }
        }
    }
}

impl fmt::Display for JwtSecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.token.iter() {
            write!(f, "{:X}", byte)?;
        }
        Ok(())
    }
}

lazy_static! {
    static ref JWT_KEY: JwtSecretKey = JwtSecretKey::new_from_file("JWT_SECRET_KEY");
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub enum Role {
    User,
    Designer,
    Admin,
}

impl Default for Role {
    fn default() -> Self {
        Role::User
    }
}

impl Role {
    const RANKING: &'static [&'static Role] = &[&Role::User, &Role::Designer, &Role::Admin];
    pub fn from_str(role: &str) -> Role {
        match role.to_ascii_lowercase().as_str() {
            "admin" => Role::Admin,
            "designer" => Role::Designer,
            _ => Role::User,
        }
    }
    pub fn lt(&self, other: &Self) -> bool {
        let a = Role::RANKING.iter().position(|&r| r == self);
        let b = Role::RANKING.iter().position(|&r| r == other);
        a < b
    }
    pub fn le(&self, other: &Self) -> bool {
        let a = Role::RANKING.iter().position(|&r| r == self);
        let b = Role::RANKING.iter().position(|&r| r == other);
        a <= b
    }
    pub fn gt(&self, other: &Self) -> bool {
        let a = Role::RANKING.iter().position(|&r| r == self);
        let b = Role::RANKING.iter().position(|&r| r == other);
        a > b
    }
    pub fn ge(&self, other: &Self) -> bool {
        let a = Role::RANKING.iter().position(|&r| r == self);
        let b = Role::RANKING.iter().position(|&r| r == other);
        a >= b
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::User => write!(f, "User"),
            Role::Admin => write!(f, "Admin"),
            Role::Designer => write!(f, "Designer"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: String,
    role: String,
    exp: usize,
}

pub fn with_auth(role: Role) -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    headers_cloned()
        .map(move |headers: HeaderMap<HeaderValue>| (role.clone(), headers))
        .and_then(authorize)
}

pub fn create_jwt(uid: &str, role: &Role) -> Result<String> {
    let expiration: i64 = Utc::now()
        .checked_add_signed(chrono::Duration::days(30))
        .expect("valid timestamp")
        .timestamp();
    let claims: Claims = Claims {
        sub: uid.to_owned(),
        role: role.to_string(),
        exp: expiration as usize,
    };
    let header: jsonwebtoken::Header = Header::new(Algorithm::HS512);
    encode(&header, &claims, &EncodingKey::from_secret(&JWT_KEY.token))
        .map_err(|_| Error::JWTTokenCreationError)
}

async fn authorize((role, headers): (Role, HeaderMap<HeaderValue>)) -> WebResult<String> {
    match jwt_from_header(&headers) {
        Ok(jwt) => {
            log::info!("JWT = {}", &jwt);
            // TODO: check if token has expired
            let decoded = decode::<Claims>(
                &jwt,
                &DecodingKey::from_secret(&JWT_KEY.token),
                &Validation::new(Algorithm::HS512),
            )
            .map_err(|_| reject::custom(Error::JWTTokenError))?;
            if role == Role::Admin && Role::from_str(&decoded.claims.role) != Role::Admin {
                return Err(reject::custom(Error::NoPermissionError));
            }
            Ok(decoded.claims.sub)
        }
        Err(e) => return Err(reject::custom(e)),
    }
}

fn jwt_from_header(headers: &HeaderMap<HeaderValue>) -> Result<String> {
    let header: &warp::http::HeaderValue = match headers.get(AUTHORIZATION) {
        Some(v) => v,
        None => return Err(Error::NoAuthHeaderError),
    };
    let auth_header: &str = match std::str::from_utf8(header.as_bytes()) {
        Ok(v) => v,
        Err(_) => return Err(Error::NoAuthHeaderError),
    };
    if !auth_header.starts_with(BEARER) {
        return Err(Error::InvalidAuthHeaderError);
    }
    Ok(auth_header.trim_start_matches(BEARER).to_owned())
}
