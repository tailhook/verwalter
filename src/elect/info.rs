use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Arc;

use rustc_serialize::hex::{FromHex, FromHexError};

use super::{Info, Id};


impl Id {
    pub fn new<S:AsRef<[u8]>>(id: S) -> Id {
        Id(id.as_ref().to_owned().into_boxed_slice())
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
        }
    }
}
