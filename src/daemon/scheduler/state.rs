use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value as Json;
use serde::{Serialize, Serializer};

use hash::hash;
use id::Id;
use super::prefetch::PrefetchInfo;
use time_util::ToMsec;
use itertools::Itertools;


#[derive(Clone, Debug, Serialize)]
pub struct Schedule {
    pub timestamp: u64,
    pub hash: String,
    pub data: Json,
    pub origin: Id,
    pub num_roles: usize,
}

#[derive(Clone, Debug, Serialize)]
pub enum FollowerState {
    Waiting,
    Fetching(String),
    Stable(Arc<Schedule>),
}

// TODO(tailhook) better serialize
#[derive(Debug)]
pub enum LeaderState {
    /// This is mutexed prefetch info to not to copy that large structure
    ///
    /// WARNING: this lock is a subject of Global Lock Ordering.
    /// Which means: if you want to lock this one and shared::SharedState
    /// you must lock SharedState first! And this one second!
    Prefetching(Instant, Mutex<PrefetchInfo>),

    Calculating,
    Stable(Arc<Schedule>),
}


// TODO(tailhook) better serialize
#[derive(Debug, Serialize)]
pub enum State {
    Unstable,
    // Follower states
    Following(Id, FollowerState),
    // TODO(tailhook)
    Leading(LeaderState),
}

impl State {
    pub fn describe(&self) -> &'static str {
        use self::State::*;
        use self::LeaderState as L;
        use self::FollowerState as F;
        match *self {
            Unstable => "unstable",
            Following(_, F::Waiting) => "follower:waiting",
            Following(_, F::Fetching(..)) => "follower:fetching",
            Following(_, F::Stable(..)) => "follower:stable",
            Leading(L::Prefetching(..)) => "leader:prefetching",
            Leading(L::Calculating) => "leader:calculating",
            Leading(L::Stable(..)) => "leader:stable",
        }
    }
}

impl Serialize for LeaderState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        unimplemented!();
    }
}

/*
impl Encodable for LeaderState {
     fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        use self::LeaderState::*;
        e.emit_enum("LeaderState", |e| {
            match *self {
                Prefetching(time_started, ref x) => {
                    e.emit_enum_variant("Prefetching", 0, 2, |e| {
                        try!(e.emit_enum_variant_arg(0, |e| {
                            time_started.to_msec().encode(e)
                        }));
                        try!(e.emit_enum_variant_arg(1, |e| {
                            x.lock().expect("buildinfo lock").encode(e)
                        }));
                        Ok(())
                    })
                }
                Calculating => {
                    e.emit_enum_variant("Calculating", 1, 0, |_| {Ok(())})
                }
                Stable(ref x) => {
                    e.emit_enum_variant("Stable", 2, 1, |e| {
                        e.emit_enum_variant_arg(0, |e| {
                            x.encode(e)
                        })
                    })
                }
            }
        })
     }
}
*/

pub fn num_roles(json: &Json) -> usize {
    (
        json.get("roles")
        .and_then(|x| x.as_object())
        .map(|x| x.keys())
    ).into_iter().chain(
        json.get("nodes")
        .and_then(|x| x.as_object())
        .map(|x| x.values().filter_map(|x|
            x.get("roles")
             .and_then(|x| x.as_object())
             .map(|x| x.keys())))
        .into_iter().flat_map(|x| x)
    ).kmerge().dedup().count()
}

pub fn from_json(json: Json) -> Result<Schedule, String> {
    let mut j = match json {
        Json::Object(ob) => ob,
        v => {
            return Err(format!("Wrong data type for schedule, data: {:?}", v));
        }
    };
    let hashvalue = j.remove("hash");
    let origin = j.remove("origin")
        .and_then(|x| x.as_str().and_then(|x| x.parse().ok()));
    let timestamp = j.remove("timestamp").and_then(|x| x.as_u64());
    let data = j.remove("data");
    match (hashvalue, timestamp, data, origin) {
        (Some(Json::String(h)), Some(t), Some(d), Some(o)) => {
            let hash = hash(d.to_string());
            if hash != h {
                Err(format!("Invalid hash {:?} data {}", h, d))
            } else {
                Ok(Schedule {
                    num_roles: num_roles(&d),
                    timestamp: t,
                    hash: h.to_string(),
                    data: d,
                    origin: o,
                })
            }
        }
        (hash, tstamp, data, origin) => {
            Err(format!("Wrong data in the schedule, \
                values: {:?} -- {:?} -- {:?} -- {:?}",
                hash, tstamp, data, origin))
        }
    }
}
