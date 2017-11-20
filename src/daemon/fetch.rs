use std::mem;
use std::sync::Arc;
use std::process::exit;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use abstract_ns;
use futures::{Future, Stream, Async};
use futures::sync::mpsc::UnboundedReceiver;
use tokio_core::reactor::Timeout;

use elect::{self, ScheduleStamp};
use id::Id;
use scheduler::{Schedule, ScheduleId};
use shared::SharedState;
use tk_easyloop::{self, timeout, timeout_at};
use void::{Void, unreachable};


const PREFETCH_MIN: u64 = elect::settings::HEARTBEAT_INTERVAL * 3/2;
const PREFETCH_MAX: u64 = PREFETCH_MIN + 60_000;


pub enum Message {
    Leader,
    Follower(Id),
    Election,
    PeerSchedule(Id, ScheduleStamp),
}

enum State {
    Unstable,
    StableLeader,
    Prefetching(Prefetch),
    Replicating(Id, Replica),
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(tag="state")]
pub enum PublicState {
    Unstable,
    StableLeader,
    Prefetching(PrefetchState),
    FollowerWaiting { leader: Id },
    Replicating { leader: Id },
    Following { leader: Id },
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PrefetchState {
    Graceful,
    Fetching,
}

pub struct Prefetch {
    start: Instant,
    state: PrefetchState,
    timeout: Timeout,
    schedules: HashMap<ScheduleId, Arc<Schedule>>,
    waiting: HashMap<ScheduleId, HashSet<Id>>,
}

pub struct Replica {

}

pub struct Fetch {
    shared: SharedState,
    chan: UnboundedReceiver<Message>,
    state: State,
}

impl Future for Fetch {
    type Item = Void;
    type Error = Void;
    fn poll(&mut self) -> Result<Async<Void>, Void> {
        use self::State::*;
        self.poll_messages().expect("input channel never fails");
        self.state = match mem::replace(&mut self.state, Unstable) {
            Unstable => Unstable,
            StableLeader => StableLeader,
            Prefetching(mut pre) => match pre.poll()? {
                Async::NotReady => Prefetching(pre),
                Async::Ready(prefetch_data) => {
                    // TODO(tailhook) send parent schedules
                    self.shared.set_parents(prefetch_data);
                    StableLeader
                }
            },
            Replicating(id, mut replica) => match replica.poll()? {
                Async::NotReady => Replicating(id, replica),
                Async::Ready(()) => unreachable!(),
            },
        };
        let state = self.public_state();
        if *self.shared.fetch_state.get() != state {
            self.shared.fetch_state.set(Arc::new(state));
        }
        Ok(Async::NotReady)
    }
}

impl Fetch {
    fn public_state(&self) -> PublicState {
        use self::State as S;
        use self::PublicState as P;
        match self.state {
            S::Unstable => P::Unstable,
            S::StableLeader => P::StableLeader,
            S::Prefetching(Prefetch { state, .. } ) => P::Prefetching(state),
            // TODO(tailhook) unpack replication state
            S::Replicating(ref id, ..)
            => P::Replicating { leader: id.clone() },
        }
    }
    fn poll_messages(&mut self) -> Result<(), ()> {
        use self::Message::*;
        use self::State::*;
        while let Async::Ready(value) = self.chan.poll()? {
            let msg = if let Some(x) = value { x }
                else {
                    error!("Premature exit of fetch channel");
                    exit(82);
                };
            match (msg, &mut self.state) {
                (Leader, &mut StableLeader) => {},
                (Leader, &mut Prefetching(..)) => {},
                (Leader, state) => {
                    // TODO(tailhook) drop schedule
                    let mut schedules = HashMap::new();
                    if let Some(sch) = self.shared.schedule() {
                        schedules.insert(sch.hash.clone(), sch);
                    }
                    let dline = Instant::now() +
                        Duration::from_millis(PREFETCH_MIN);
                    *state = Prefetching(Prefetch {
                        start: Instant::now(),
                        state: PrefetchState::Graceful,
                        timeout: timeout_at(dline),
                        schedules,
                        waiting: HashMap::new(),
                    });
                }
                (Follower(ref d), &mut Replicating(ref s, ..)) if s == d => {}
                (Follower(id), state) => {
                    // TODO(tailhook) drop schedule
                    *state = Replicating(id, Replica {
                    });
                }
                (Election, &mut Unstable) => {}
                (Election, state) => {
                    // TODO(tailhook) drop schedule
                    *state = Unstable;
                }
                (PeerSchedule(_, _), &mut Unstable) => {} // ignore
                (PeerSchedule(_, _), &mut StableLeader) => {} // ignore
                (PeerSchedule(ref id, ref stamp),
                 &mut Prefetching(ref mut pre))
                => {
                    unimplemented!()
                    // pre.report(id, stamp)
                }
                (PeerSchedule(ref d, ref stamp), &mut Replicating(ref s, ref mut rep))
                if s == d
                => {
                    // TODO(tailhook) drop schedule
                    unimplemented!();
                    // rep.update(stamp)
                }
                (PeerSchedule(id, stamp), state@&mut Replicating(..)) => {
                    // TODO(tailhook) drop schedule
                    *state = Replicating(id, Replica {
                    });
                }
            }
        }
        Ok(())
    }
}

impl Future for Replica {
    type Item = ();
    type Error = Void;
    fn poll(&mut self) -> Result<Async<()>, Void> {
        unimplemented!()
    }
}

impl Prefetch {
    fn get_state(&mut self) -> Vec<Arc<Schedule>> {
        self.schedules.drain().map(|(_, v)| v).collect()
    }
}

impl Future for Prefetch {
    type Item = Vec<Arc<Schedule>>;
    type Error = Void;
    fn poll(&mut self) -> Result<Async<Vec<Arc<Schedule>>>, Void> {

        if self.state == PrefetchState::Fetching && self.waiting.len() == 0 {
            return Ok(Async::Ready(self.get_state()));
        }
        match self.timeout.poll().expect("timeout never fails") {
            Async::Ready(()) if self.state == PrefetchState::Graceful => {
                self.state = PrefetchState::Fetching;
                self.timeout = timeout_at(self.start +
                    Duration::from_millis(PREFETCH_MAX));
                match self.timeout.poll().expect("timeout never fails") {
                    Async::Ready(()) => {
                        return Ok(Async::Ready(self.get_state()));
                    }
                    Async::NotReady => {}
                }
            }
            Async::Ready(()) => {
                return Ok(Async::Ready(self.get_state()));
            }
            Async::NotReady => {}
        }
        Ok(Async::NotReady)
    }
}

pub fn spawn_fetcher(ns: &abstract_ns::Router, state: &SharedState,
    chan: UnboundedReceiver<Message>)
    -> Result<(), Box<::std::error::Error>>
{
    tk_easyloop::spawn(Fetch {
        shared: state.clone(),
        chan: chan,
        state: State::Unstable,
    }.map(|v| unreachable(v)).map_err(|e| unreachable(e)));
    Ok(())
}
