use std::time::{SystemTime, Duration, UNIX_EPOCH};

use serde::Serializer;


fn tstamp_to_ms(tm: SystemTime) -> u64 {
    let ts = tm.duration_since(UNIX_EPOCH)
        .expect("timestamp is always after unix epoch");
    return ts.as_secs()*1000 + (ts.subsec_nanos() / 1000000) as u64;
}

fn duration_to_ms(dur: Duration) -> u64 {
    return dur.as_secs()*1000 + (dur.subsec_nanos() / 1000000) as u64;
}

pub fn serialize_timestamp<S>(tm: &SystemTime, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
{
    ser.serialize_u64(tstamp_to_ms(*tm))
}

pub fn serialize_opt_timestamp<S>(tm: &Option<SystemTime>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
{
    match *tm {
        Some(ts) => {
            ser.serialize_some(&tstamp_to_ms(ts))
        }
        None => {
            ser.serialize_none()
        }
    }
}


pub fn serialize_duration<S>(tm: &Duration, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
{
    ser.serialize_u64(duration_to_ms(*tm))
}
