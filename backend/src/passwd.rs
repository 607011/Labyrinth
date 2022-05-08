use crate::error::Error;
use argon2::{self, Config, ThreadMode, Variant, Version};
use rand;

pub struct Password {}

type Result<T> = std::result::Result<T, Error>;

impl Password {
    pub fn hash(password: &String) -> Result<String> {
        let config: argon2::Config = Config {
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
        match argon2::hash_encoded(password.as_bytes(), &salt, &config) {
            Ok(hash) => Ok(hash),
            Err(_) => return Err(Error::HashingError),
        }
    }
    pub fn matches(hash: &String, password: &String) -> Result<bool> {
        match argon2::verify_encoded(hash, password.as_bytes()) {
            Ok(matches) => Ok(matches),
            Err(_) => return Err(Error::HashingError),
        }
    }
}
