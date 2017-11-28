use std::io::{Write, Read};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use rustc_serialize::hex::{FromHex, ToHex, FromHexError};
use rustc_serialize::{Encodable, Encoder as RustcEncoder};
use serde::{Serialize, Serializer};


#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Id(InternalId);

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum InternalId {
    Good([u8; 16]),
    Bad(Arc<Box<[u8]>>),
}

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
            InternalId::Good(ar) => ar.to_hex(),
            InternalId::Bad(ref vec) => vec.to_hex(),
        }
    }
}

impl Encodable for Id {
    fn encode<S: RustcEncoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.to_hex().encode(s)
    }
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        self.to_hex().serialize(serializer)
    }
}

impl FromStr for Id {
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Id, Self::Err> {
        s.from_hex().map(Id::new)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            InternalId::Good(ar) => {
                write!(fmt, "{}", ar.to_hex())
            }
            InternalId::Bad(ref vec) => {
                write!(fmt, "{}", vec.to_hex())
            }
        }
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            InternalId::Good(ar) => {
                write!(fmt, "Id({})", ar.to_hex())
            }
            InternalId::Bad(ref vec) => {
                write!(fmt, "Id({})", vec.to_hex())
            }
        }
    }
}
