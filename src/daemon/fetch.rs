use abstract_ns;
use futures::{Future, Async};
use futures::sync::mpsc::UnboundedReceiver;

use id::Id;
use shared::SharedState;
use elect::ScheduleStamp;
use tk_easyloop;


pub enum Message {
    Leader,
    Follower(Id),
    Unstable,
    PeerSchedule(Id, ScheduleStamp),
}

pub struct Fetch {
    shared: SharedState,
    chan: UnboundedReceiver<Message>,
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
    });
    Ok(())
}
