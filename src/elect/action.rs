use rotor::Time;

use super::{Id};


#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    PingAll,
    Vote(Id),
    ConfirmVote(Id),
    Pong,
    PingNew,
}

#[derive(PartialEq, Eq, Debug)]
pub struct ActionList {
    pub next_wakeup: Time,
    pub action: Option<Action>,
}

impl Action {
    pub fn and_wait(self, time: Time) -> ActionList {
        ActionList {
            next_wakeup: time,
            action: Some(self),
        }
    }
    pub fn wait(time: Time) -> ActionList {
        ActionList {
            next_wakeup: time,
            action: None,
        }
    }
}

