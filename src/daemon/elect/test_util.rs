use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::net;
use std::time::{Duration, SystemTime, Instant};
use std::collections::HashMap;

use crossbeam::atomic::ArcCell;
use super::{Info};
use id::Id;
use peer::Peer;

static NODE_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;


pub struct Environ {
    pub id: Id,
    all_hosts: HashMap<Id, ArcCell<Peer>>,
    now: Instant,  // TODO(tailhook) should `now` and `tspec` be unified?
    tspec: Instant,
}

impl Environ {
    pub fn new(id: &str) -> Environ {
        Environ {
            id: id.parse().unwrap(),
            all_hosts: vec![].into_iter().collect(),
            now: Instant::now(),
            tspec: Instant::now(),
        }
    }
    pub fn info<'x>(&'x self) -> Info<'x> {
        Info {
            id: &self.id,
            hosts_timestamp: Some(SystemTime::now()),  // TODO(tailhook)
            all_hosts: &self.all_hosts,
            debug_force_leader: false,
            allow_minority: true,
        }
    }
    pub fn sleep(&mut self, ms: u64) {
        self.now = self.now +  Duration::from_millis(ms);
        self.tspec = self.tspec + Duration::from_millis(ms);
    }
    /// A single tick in mio is 100ms AFAIK. This is convenience method
    /// to have some time passed
    pub fn tick(&mut self) {
        self.sleep(100)
    }
    pub fn now(&self) -> Instant {
        self.now
    }
    pub fn add_node(&mut self) -> Id {
        let n = NODE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let id: Id = format!("e0beef{:02x}", n).parse().unwrap();
        self.all_hosts.insert(id.clone(), ArcCell::new(Arc::new(Peer {
            id: id.clone(),
            addr: Some(net::SocketAddr::V4(net::SocketAddrV4::new(
                net::Ipv4Addr::new(127, 0, (n >> 8) as u8, (n & 0xFF) as u8),
                12345))),
            hostname: format!("{}", id),
            name: format!("{}", id),
            schedule: None,
            last_report_direct: Some(SystemTime::now()),
            known_since: SystemTime::now(),
            errors: 0,
        })));
        id
    }
}
