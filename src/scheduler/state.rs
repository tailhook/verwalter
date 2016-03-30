use std::sync::{Arc, Mutex};

use rustc_serialize::json::Json;
use rustc_serialize::{Encodable, Encoder};

use shared::Id;
use super::prefetch::PrefetchInfo;


#[derive(Clone, Debug, RustcEncodable)]
pub struct Schedule {
    pub timestamp: u64,
    pub hash: String,
    pub data: Json,
    pub origin: Id,
}

#[derive(Clone, Debug, RustcEncodable)]
pub enum FollowerState {
    Waiting,
    Fetching(String),
    Stable(Arc<Schedule>),
}

#[derive(Debug)]
pub enum LeaderState {
    /// This is mutexed prefetch info to not to copy that large structure
    ///
    /// WARNING: this lock is a subject of Global Lock Ordering.
    /// Which means: if you want to lock this one and shared::SharedState
    /// you must lock SharedState first! And this one second!
    Prefetching(Mutex<PrefetchInfo>),

    Calculating,
    Stable(Arc<Schedule>),
}


#[derive(Debug, RustcEncodable)]
pub enum State {
    Unstable,
    // Follower states
    Following(Id, FollowerState),
    Leading(LeaderState),
}

impl Encodable for LeaderState {
     fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        use self::LeaderState::*;
        e.emit_enum("LeaderState", |e| {
            match *self {
                Prefetching(ref x) => {
                    e.emit_enum_variant("Prefetching", 0, 1, |e| {
                        e.emit_enum_variant_arg(0, |e| {
                            x.lock().expect("buildinfo lock").encode(e)
                        })
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
