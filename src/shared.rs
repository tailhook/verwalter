use std::io::{Read, Write};
use std::str::FromStr;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, Condvar};
use std::collections::HashMap;

use time::Timespec;
use rotor::{Time, Notifier};
use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use rustc_serialize::hex::{FromHex, ToHex, FromHexError};
use rustc_serialize::json::Json;
use rustc_serialize::{Encodable, Encoder as RustcEncoder};

use config::Config;
use elect::ElectionState;


#[derive(Clone)]
pub struct SharedState(Arc<Mutex<State>>, Arc<Notifiers>);


#[derive(Clone, PartialEq, Eq, Hash)]
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

impl Encodable for Id {
    fn encode<S: RustcEncoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.to_hex().encode(s)
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

impl ::std::fmt::Debug for Id {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(fmt, "Id({})", self.0.to_hex())
    }
}

#[derive(Clone, Debug)]
pub struct Peer {
     pub addr: Option<SocketAddr>,
     pub hostname: String,
     pub last_report: Option<Timespec>,
}

#[derive(Clone, Debug, RustcEncodable)]
pub struct Schedule {
    pub timestamp: u64,
    pub hash: String,
    pub data: Json,
    pub origin: bool,
}

#[derive(Debug)]
struct State {
    config: Arc<Config>,
    peers: Option<Arc<(Time, HashMap<Id, Peer>)>>,
    schedule: Option<Arc<Schedule>>,
    election: Arc<ElectionState>,
    target_schedule_hash: Option<String>,
    election_update: Option<Notifier>,
}

struct Notifiers {
    apply_schedule: Condvar,
}

impl SharedState {
    pub fn new(cfg: Config) -> SharedState {
        SharedState(Arc::new(Mutex::new(State {
            config: Arc::new(cfg),
            peers: None,
            schedule: None,
            election: Default::default(),
            target_schedule_hash: None,
            election_update: None, //unfortunately
        })), Arc::new(Notifiers {
            apply_schedule: Condvar::new(),
        }))
    }
    // Accessors
    pub fn peers(&self) -> Option<Arc<(Time, HashMap<Id, Peer>)>> {
        self.0.lock().expect("shared state lock").peers.clone()
    }
    pub fn config(&self) -> Arc<Config> {
        self.0.lock().expect("shared state lock").config.clone()
    }
    pub fn schedule(&self) -> Option<Arc<Schedule>> {
        self.0.lock().expect("shared state lock").schedule.clone()
    }
    pub fn election(&self) -> Arc<ElectionState> {
        self.0.lock().expect("shared state lock").election.clone()
    }
    // Setters
    pub fn set_peers(&self, time: Time, peers: HashMap<Id, Peer>) {
        self.0.lock().expect("shared state lock")
            .peers = Some(Arc::new((time, peers)));
    }
    #[allow(unused)]
    pub fn set_config(&self, cfg: Config) {
        self.0.lock().expect("shared state lock").config = Arc::new(cfg);
    }
    pub fn set_schedule(&self, val: Schedule) {
        let mut guard = self.0.lock().expect("shared state lock");
        guard.schedule = Some(Arc::new(val));
        self.1.apply_schedule.notify_all();
    }
    pub fn set_schedule_if_matches(&self, val: Schedule) {
        let mut guard = self.0.lock().expect("shared state lock");
        if guard.target_schedule_hash.as_ref() == Some(&val.hash) {
            guard.schedule = Some(Arc::new(val));
            self.1.apply_schedule.notify_all();
        }
    }
    pub fn set_election(&self, val: ElectionState) {
        let mut guard = self.0.lock().expect("shared state lock");
        if !val.is_stable {
            guard.target_schedule_hash = None;
        }
        guard.election = Arc::new(val);
        guard.election_update.as_mut()
            .map(|x| x.wakeup().expect("election update notify"));
    }
    pub fn set_target_schedule(&self, hash: String) {
        info!("Set target schedule to {}", hash);
        let mut guard = self.0.lock().expect("shared state lock");
        guard.target_schedule_hash = Some(hash);
        guard.election_update.as_mut()
            .map(|x| x.wakeup().expect("election update notify"));
    }
    // Utility
    pub fn wait_new_schedule(&self, hash: &str) -> Arc<Schedule> {
        let mut guard = self.0.lock().expect("shared state lock");
        if guard.schedule.as_ref().map(|x| x.hash != hash).unwrap_or(false) {
            return guard.schedule.clone().unwrap();
        }
        loop {
            guard = self.1.apply_schedule.wait(guard)
                .expect("shared state lock");
            match guard.target_schedule_hash {
                Some(ref thash) => {
                    if thash == &hash { // already up to date
                        continue;
                    }
                    if guard.schedule.as_ref().map(|x| &x.hash == thash)
                        .unwrap_or(false)
                    {
                        return guard.schedule.clone().unwrap();
                    }
                }
                None => continue,
            };
        }
    }
    pub fn set_update_notifier(&self, notifier: Notifier) {
        let mut guard = self.0.lock().expect("shared state lock");
        guard.election_update = Some(notifier);
    }
}
