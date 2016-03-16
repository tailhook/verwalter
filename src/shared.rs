use std::io::{Read, Write};
use std::str::FromStr;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use time::Timespec;
use rotor::Time;
use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use rustc_serialize::hex::{FromHex, ToHex, FromHexError};

use config::Config;


#[derive(Clone, Debug)]
pub struct SharedState(Arc<Mutex<State>>);


#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Id(Box<[u8]>);

impl Id {
    pub fn new<S:AsRef<[u8]>>(id: S) -> Id {
        Id(id.as_ref().to_owned().into_boxed_slice())
    }
    pub fn encode<W: Write>(&self, enc: &mut Encoder<W>) -> EncodeResult {
        enc.bytes(&self.0[..])
    }
    pub fn decode<R: Read>(dec: &mut Decoder<R>) -> DecodeResult<Id> {
        dec.bytes().map(|x| x.into_boxed_slice()).map(Id)
    }
    pub fn to_hex(&self) -> String {
        return self.0[..].to_hex();
    }
}

impl FromStr for Id {
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Id, Self::Err> {
        s.from_hex().map(|x| x.into_boxed_slice()).map(Id)
    }
}

impl ::std::fmt::Display for Id {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(fmt, "{}", self.0.to_hex())
    }
}

#[derive(Clone, Debug)]
pub struct Peer {
     pub addr: Option<SocketAddr>,
     pub hostname: String,
     pub last_report: Option<Timespec>,
}

#[derive(Debug)]
struct State {
    config: Arc<Config>,
    peers: Option<Arc<(Time, HashMap<Id, Peer>)>>,
}

impl SharedState {
    pub fn new(cfg: Config) -> SharedState {
        SharedState(Arc::new(Mutex::new(State {
            config: Arc::new(cfg),
            peers: None,
        })))
    }
    pub fn peers(&self) -> Option<Arc<(Time, HashMap<Id, Peer>)>> {
        self.0.lock().expect("shared state lock").peers.clone()
    }
    pub fn config(&self) -> Arc<Config> {
        self.0.lock().expect("shared state lock").config.clone()
    }
    pub fn set_peers(&self, time: Time, peers: HashMap<Id, Peer>) {
        self.0.lock().expect("shared state lock")
            .peers = Some(Arc::new((time, peers)));
    }
    #[allow(unused)]
    pub fn set_config(&self, cfg: Config) {
        self.0.lock().expect("shared state lock").config = Arc::new(cfg);
    }
}
