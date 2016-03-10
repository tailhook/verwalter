use std::io::Write;
use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cbor::{Encoder, EncodeResult};
use rotor::Time;
use rustc_serialize::hex::{FromHex, FromHexError};

use super::{Info, Id, peers_refresh};


impl Id {
    pub fn new<S:AsRef<[u8]>>(id: S) -> Id {
        Id(id.as_ref().to_owned().into_boxed_slice())
    }
    pub fn encode<W: Write>(&self, enc: &mut Encoder<W>) -> EncodeResult {
        enc.bytes(&self.0[..])
    }
}

impl FromStr for Id {
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Id, Self::Err> {
        s.from_hex().map(|x| x.into_boxed_slice()).map(Id)
    }
}

impl Info {
    pub fn new(id: Id) -> Info {
        Info {
            id: id,
            all_hosts: HashMap::new(),
            hosts_timestamp: None,
        }
    }
    pub fn hosts_are_fresh(&self, now: Time) -> bool {
        self.hosts_timestamp
            .map(|x| x + peers_refresh()*3/2 > now)
            .unwrap_or(false)
    }
}
