use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
    let encoded: String = base64::encode(v);
    String::serialize(&encoded, s)
}
pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
    let decoded: String = String::deserialize(d)?;
    base64::decode(decoded.as_bytes()).map_err(|e| serde::de::Error::custom(e))
}
