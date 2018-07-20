use std::io::{Write, Read};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use juniper::Value;
use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use hex::{FromHex, ToHex, encode as hexlify, FromHexError};
use serde::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};


#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Id(InternalId);

graphql_scalar!(Id {
    description: "Node id"
    resolve(&self) -> Value {
        Value::string(self.to_string())
    }

    from_input_value(_v: &InputValue) -> Option<Id> {
        unimplemented!();
    }
});

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum InternalId {
    Good([u8; 16]),
    Bad(Arc<Box<[u8]>>),
}

struct IdVisitor;

impl Id {
    pub fn new<S:AsRef<[u8]>>(id: S) -> Id {
        let id = id.as_ref();
        if id.len() == 16 {
            let mut x = [0u8; 16];
            x.copy_from_slice(id);
            Id(InternalId::Good(x))
        } else {
            Id(InternalId::Bad(Arc::new(id.to_vec().into_boxed_slice())))
        }
    }
    pub fn encode_cbor<W: Write>(&self, enc: &mut Encoder<W>) -> EncodeResult {
        match self.0 {
            InternalId::Good(ar) => enc.bytes(&ar[..]),
            InternalId::Bad(ref vec) => enc.bytes(&*vec),
        }
    }
    pub fn decode<R: Read>(dec: &mut Decoder<R>) -> DecodeResult<Id> {
        dec.bytes().map(Id::new)
    }
    pub fn to_hex(&self) -> String {
        match self.0 {
            InternalId::Good(ar) => hexlify(ar),
            InternalId::Bad(ref vec) => hexlify(&vec[..]),
        }
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        use self::InternalId::*;
        // TODO(tailhook) bytes on is_human_readable() == false
        if serializer.is_human_readable() {
            self.to_hex().serialize(serializer)
        } else {
            match self.0 {
                Good(ar) => serializer.serialize_bytes(&ar),
                Bad(ref vec) => serializer.serialize_bytes(&vec[..]),
            }
        }
    }
}

impl<'de> Visitor<'de> for IdVisitor {
    type Value = Id;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("bytes or string")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        s.parse().map_err(|e| E::custom(e))
    }
    fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Id::new(bytes))
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_bytes(IdVisitor)
    }
}

impl FromStr for Id {
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Id, Self::Err> {
        let ar: Vec<u8> = FromHex::from_hex(s.as_bytes())?;
        if ar.len() == 16 {
            Ok(Id::new(ar))
        } else {
            Ok(Id(InternalId::Bad(Arc::new(ar.into()))))
        }
    }
}

impl fmt::Display for Id {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            InternalId::Good(ar) => {
                ar.write_hex(fmt)
            }
            InternalId::Bad(ref vec) => {
                vec.write_hex(fmt)
            }
        }
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            InternalId::Good(ar) => {
                write!(fmt, "Id({})", hexlify(ar))
            }
            InternalId::Bad(ref vec) => {
                write!(fmt, "Id({})", hexlify(&vec[..]))
            }
        }
    }
}
