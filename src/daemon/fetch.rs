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
        unimplemented!();
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
