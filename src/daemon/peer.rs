use std::net::SocketAddr;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam::sync::ArcCell;
use elect::ScheduleStamp;
use id::Id;


#[derive(Clone, Debug, Serialize)]
pub struct Peer {
    pub addr: Option<SocketAddr>,
    pub name: String,
    pub hostname: String,
    #[serde(skip_deserializing)]
    pub schedule: Option<ScheduleStamp>,
    pub known_since: SystemTime,
    pub last_report_direct: Option<SystemTime>,
}

#[derive(Debug)]
pub struct Peers {
    pub timestamp: SystemTime,
    pub peers: HashMap<Id, ArcCell<Peer>>,
}

impl Peers {
    pub fn new() -> Peers {
        Peers {
            timestamp: UNIX_EPOCH,
            peers: HashMap::new(),
        }
    }
}
