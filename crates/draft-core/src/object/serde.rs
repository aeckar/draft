use std::{collections::HashMap, fmt};

use ordered_float::NotNan;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, SeqAccess, Visitor},
};

use crate::object::Object;

impl Serialize for Object {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(b) => serializer.serialize_bool(*b),
            Self::Number(n) => serializer.serialize_f64(n.into_inner()),
            Self::String(s) => serializer.serialize_str(s),
            Self::List(items) => items.serialize(serializer),
            Self::Map { map } => map.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ObjectVisitor;

        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = Object;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid object notation primitive or structural type")
            }

            fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
                Ok(Object::Bool(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                NotNan::new(v as f64)
                    .map(Object::Number)
                    .map_err(|_| serde::de::Error::custom("Invalid float conversion"))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                NotNan::new(v as f64)
                    .map(Object::Number)
                    .map_err(|_| serde::de::Error::custom("Invalid float conversion"))
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                NotNan::new(v).map(Object::Number).map_err(|_| {
                    serde::de::Error::custom("NaN values are not permitted inside Object::Number")
                })
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Object::String(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(Object::String(v))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Object::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Object::Null)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                while let Some(element) = seq.next_element()? {
                    vec.push(element);
                }
                Ok(Object::List(vec))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut hashmap = HashMap::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((key, value)) = map.next_entry()? {
                    hashmap.insert(key, value);
                }
                Ok(Object::Map { map: hashmap })
            }
        }

        deserializer.deserialize_any(ObjectVisitor)
    }
}
