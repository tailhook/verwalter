use std::collections::{HashSet, BinaryHeap};
use std::cmp::Ordering;
use std::net::SocketAddr;
use std::time::Instant;

use futures::{Future, Async};
use tokio_core::reactor::{Handle, Timeout};


pub(crate) struct Blacklist {
    addrs: HashSet<SocketAddr>,
    heap: BinaryHeap<Pair>,
    timeout: Option<Timeout>,
    handle: Handle,
}

#[derive(Eq)]
pub struct Pair(Instant, SocketAddr);

impl PartialOrd for Pair {
    fn partial_cmp(&self, other: &Pair) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Pair {
    fn cmp(&self, other: &Pair) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialEq for Pair {
    fn eq(&self, other: &Pair) -> bool {
        self.0.eq(&other.0)
    }
}

impl Blacklist {
    pub fn new(h: &Handle) -> Blacklist {
        Blacklist {
            addrs: HashSet::new(),
            heap: BinaryHeap::new(),
            timeout: None,
            handle: h.clone(),
        }
    }
    pub fn blacklist(&mut self, addr: SocketAddr, time: Instant) {
        if self.addrs.contains(&addr) {
            // can't add again because is in heap
            return;
        }
        self.heap.push(Pair(time, addr));
        self.addrs.insert(addr);
    }
    pub fn is_failing(&self, addr: SocketAddr) -> bool {
        return self.addrs.contains(&addr);
    }
    pub fn poll(&mut self) -> Async<SocketAddr> {
        loop {
            match self.heap.peek() {
                Some(&Pair(time, a)) if time <= Instant::now() => {
                    self.timeout = None;
                    self.addrs.remove(&a);
                    self.heap.pop();
                    return Async::Ready(a);
                }
                Some(&Pair(time, _)) => {
                    let timer_result = self.timeout.as_mut()
                        .map(|x| x.poll().expect("timeout never fails"));
                    match timer_result {
                        Some(Async::NotReady) => return Async::NotReady,
                        _ => {
                            self.timeout = None;
                        }
                    }
                    let mut timer = Timeout::new_at(time, &self.handle)
                        .expect("timeout never fails");
                    match timer.poll().expect("timeout never fails") {
                        Async::Ready(()) => continue,
                        Async::NotReady => {
                            self.timeout = Some(timer);
                            return Async::NotReady;
                        }
                    }
                }
                None => {
                    self.timeout = None;
                    return Async::NotReady;
                }
            }
        }
    }
}
