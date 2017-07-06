use std::sync::Arc;

use abstract_ns;
use futures::{Future, Async};
use futures::sync::mpsc::UnboundedReceiver;

use elect::ScheduleStamp;
use id::Id;
use scheduler::Schedule;
use shared::SharedState;
use tk_easyloop;


pub enum Message {
    Leader,
    Follower(Id),
    Unstable,
    PeerSchedule(Id, ScheduleStamp),
}

enum State {
    Unstable,
    StableLeader,
    Prefetching(Prefetch),
    FollowerWaiting(Id),
    Replicating(Id, Replica),
    Following(Id, Arc<Schedule>),
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
        self.poll_messages()?;
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
            S::FollowerWaiting(ref id)
            => P::FollowerWaiting {leader: id.clone()},
            S::Replicating(ref id, ..)
            => P::Replicating { leader: id.clone() },
            S::Following(ref id, ..)
            => P::Following { leader: id.clone() },
        }
    }
    fn poll_messages(&mut self) -> Result<(), ()> {
        Ok(())
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
