use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::net;
use std::time::Duration;
use std::collections::HashMap;

use rotor::Time;
use time::{Timespec, Duration as Dur, get_time};

use super::{Info};
use shared::{Id, Peer};

static NODE_COUNTER: AtomicUsize = ATOMIC_USIZE_INIT;


pub struct Environ {
    pub id: Id,
    all_hosts: HashMap<Id, Peer>,
    now: Time,
    tspec: Timespec,
}

impl Environ {
    pub fn new(id: &str) -> Environ {
        Environ {
            id: id.parse().unwrap(),
            all_hosts: vec![(id.parse().unwrap(), Peer {
                addr: Some(net::SocketAddr::V4(net::SocketAddrV4::new(
                    net::Ipv4Addr::new(127, 0, 0, 1),
                    12345))),
                hostname: format!("{}", id),
                name: format!("{}", id),
                last_report: Some(get_time()),
            })].into_iter().collect(),
            now: Time::zero(),
            tspec: get_time(),
        }
    }
    pub fn info<'x>(&'x self) -> Info<'x> {
        Info {
            id: &self.id,
            hosts_timestamp: Some(self.now),  // TODO(tailhook)
            all_hosts: &self.all_hosts,
            debug_force_leader: false,
        }
    }
    pub fn sleep(&mut self, ms: u64) {
        self.now = self.now +  Duration::from_millis(ms);
        self.tspec = self.tspec + Dur::milliseconds(ms as i64);
    }
    /// A single tick in mio is 100ms AFAIK. This is convenience method
    /// to have some time passed
    pub fn tick(&mut self) {
        self.sleep(100)
    }
    pub fn now(&self) -> Time {
        self.now
    }
    pub fn add_node(&mut self) -> Id {
        let n = NODE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let id: Id = format!("e0beef{:02x}", n).parse().unwrap();
        self.all_hosts.insert(id.clone(), Peer {
            addr: Some(net::SocketAddr::V4(net::SocketAddrV4::new(
                net::Ipv4Addr::new(127, 0, (n >> 8) as u8, (n & 0xFF) as u8),
                12345))),
            hostname: format!("{}", id),
            name: format!("{}", id),
            last_report: Some(self.tspec),
        });
        id
    }
}
