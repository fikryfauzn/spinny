//! Preserve duplicate-key errors before binding strict typed schemas.
use super::{FormatError, FormatErrorKind as K, FormatResult};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use std::{collections::BTreeSet, fmt};

struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("V1 JSON without duplicate keys, nulls or floating point numbers")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(Value::Number(Number::from(v))))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                if v < 0 {
                    return Err(E::custom("negative integer"));
                }
                Ok(Unique(Value::Number(Number::from(v))))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(Value::String(v.to_owned())))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(Value::String(v)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(v)) = a.next_element()? {
                    values.push(v);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut keys = BTreeSet::new();
                let mut values = Map::new();
                while let Some(key) = a.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        return Err(de::Error::custom(format!("duplicate key: {key}")));
                    }
                    let Unique(v) = a.next_value()?;
                    values.insert(key, v);
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        d.deserialize_any(JsonVisitor)
    }
}

pub(crate) fn parse(bytes: &[u8], limit: u64) -> FormatResult<Value> {
    if bytes.len() as u64 > limit {
        return Err(FormatError::new(
            K::ResourceLimit,
            "JSON exceeds byte limit",
        ));
    }
    let mut quoted = false;
    let mut escaped = false;
    for &byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte == b'-' {
            return Err(FormatError::new(K::InvalidJson, "signed integer token"));
        }
    }
    let Unique(value) = serde_json::from_slice(bytes)
        .map_err(|e| FormatError::new(K::InvalidJson, e.to_string()))?;
    if !value.is_object() {
        return Err(FormatError::new(
            K::InvalidSchema,
            "JSON root must be an object",
        ));
    }
    Ok(value)
}

pub(crate) fn bind<T: serde::de::DeserializeOwned>(value: Value) -> FormatResult<T> {
    serde_json::from_value(value).map_err(|e| FormatError::new(K::InvalidSchema, e.to_string()))
}
