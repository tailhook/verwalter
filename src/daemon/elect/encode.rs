use std::sync::Arc;

use serde_cbor::{to_vec, from_slice};

use failure::Error;
use id::{Id};
use elect::{Capsule, ScheduleStamp};
use elect::machine::Epoch;
use scheduler::Schedule;
use frontend::serialize::ms_to_system_time;


#[derive(Debug, Serialize, Deserialize)]
#[serde(tag="type")]
pub enum Packet {
    Ping {
        id: Id,
        epoch: Epoch,
        schedule: Option<ScheduleStamp>,
    },
    Pong {
        id: Id,
        epoch: Epoch,
        schedule: Option<ScheduleStamp>,
        errors: usize,
    },
    Vote {
        id: Id,
        epoch: Epoch,
        target: Id,
        schedule: Option<ScheduleStamp>,
    },
}

impl Packet {
    pub fn to_vec(&self) -> Vec<u8> {
        to_vec(self).expect("can serialize packet")
    }
}

impl ScheduleStamp {
    pub fn from(schedule: &Arc<Schedule>) -> ScheduleStamp {
        ScheduleStamp {
            timestamp: ms_to_system_time(schedule.timestamp),
            hash: schedule.hash.clone(),
            origin: schedule.origin.clone(),
        }
    }
}

pub fn read_packet(buf: &[u8]) -> Result<Capsule, Error> {
    use elect::Message::*;
    let pkt: Packet = from_slice(buf)?;
    let result = match pkt {
        Packet::Ping { id, epoch, schedule } => {
            let source = id;
            Capsule { source, epoch, schedule, message: Ping, errors: None }
        }
        Packet::Pong { id, epoch, schedule, errors } => {
            let source = id;
            let errors = Some(errors);
            Capsule { source, epoch, schedule, message: Pong, errors }
        }
        Packet::Vote { id, epoch, target, schedule } => {
            let source = id;
            let message = Vote(target);
            Capsule { source, epoch, schedule, message, errors: None }
        }
    };
    return Ok(result);
}
