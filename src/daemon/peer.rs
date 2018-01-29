use std::net::SocketAddr;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam::sync::ArcCell;
use elect::ScheduleStamp;
use id::Id;
use serde_millis;


#[derive(Clone, Debug, Serialize)]
pub struct Peer {
    pub addr: Option<SocketAddr>,
    pub name: String,
    pub hostname: String,
    #[serde(skip_deserializing)]
    pub schedule: Option<ScheduleStamp>,
    #[serde(with="serde_millis")]
    pub known_since: SystemTime,
    #[serde(with="serde_millis")]
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

impl Peer {
    pub fn needs_refresh(&self, remote: &Peer) -> bool {
        let &Peer {
            ref addr, ref name, ref hostname,
            ref known_since, ref last_report_direct,
            schedule: _,  // ensure that only schedule is skipped
        } = self;
        return addr != &remote.addr || name != &remote.name ||
           hostname != &remote.hostname || known_since != &remote.known_since ||
           last_report_direct != &remote.last_report_direct;
    }
}
