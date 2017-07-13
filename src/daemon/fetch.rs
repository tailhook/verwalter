use std::mem;
use std::sync::Arc;

use abstract_ns;
use futures::{Future, Stream, Async};
use futures::sync::mpsc::UnboundedReceiver;

use elect::ScheduleStamp;
use id::Id;
use scheduler::Schedule;
use shared::SharedState;
use tk_easyloop;


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
    Prefetching,
    FollowerWaiting { leader: Id },
    Replicating { leader: Id },
    Following { leader: Id },
}

pub struct Prefetch {
}

pub struct Replica {

}

pub struct Fetch {
    shared: SharedState,
    chan: UnboundedReceiver<Message>,
    state: State,
}

impl Future for Fetch {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        use self::State::*;
        self.poll_messages()?;
        self.state = match mem::replace(&mut self.state, Unstable) {
            Unstable => Unstable,
            StableLeader => StableLeader,
            Prefetching(mut pre) => match pre.poll()? {
                Async::NotReady => Prefetching(pre),
                Async::Ready(prefetch_data) => {
                    // TODO(tailhook) send parent schedules
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
            S::Prefetching(..) => P::Prefetching,
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
                    // TODO(tailhook) panic?
                    return Err(());
                };
            match (msg, &mut self.state) {
                (Leader, &mut StableLeader) => {},
                (Leader, state) => {
                    // TODO(tailhook) drop schedule
                    *state = Prefetching(Prefetch {
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
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!()
    }
}

impl Future for Prefetch {
    type Item = Vec<Arc<Schedule>>;
    type Error = ();
    fn poll(&mut self) -> Result<Async<Vec<Arc<Schedule>>>, ()> {
        unimplemented!()
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
    });
    Ok(())
}
