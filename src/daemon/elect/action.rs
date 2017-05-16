use std::time::{Instant, Duration};

use id::Id;


#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    PingAll,
    Vote(Id),
    ConfirmVote(Id),
    Pong(Id),
}

#[derive(PartialEq, Eq, Debug)]
pub struct ActionList {
    pub next_wakeup: Instant,
    pub action: Option<Action>,
}

impl Action {
    pub fn and_wait(self, time: Instant) -> ActionList {
        ActionList {
            next_wakeup: time,
            action: Some(self),
        }
    }
    pub fn wait(time: Instant) -> ActionList {
        ActionList {
            next_wakeup: time,
            action: None,
        }
    }
}

