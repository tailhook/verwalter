use std::time::{SystemTime, Duration, Instant, UNIX_EPOCH};

use serde::Serializer;
use serde::de::{self, Deserialize, Deserializer};


fn tstamp_to_ms(tm: SystemTime) -> u64 {
    let ts = tm.duration_since(UNIX_EPOCH)
        .expect("timestamp is always after unix epoch");
    return ts.as_secs()*1000 + (ts.subsec_nanos() / 1000000) as u64;
}

fn instant_to_ms(tm: Instant) -> u64 {
    let st = SystemTime::now();
    let inst = Instant::now();
    if tm > inst {
        return tstamp_to_ms(st + (tm - inst));
    } else {
        return tstamp_to_ms(st - (inst - tm));
    }
}

fn duration_to_ms(dur: Duration) -> u64 {
    return dur.as_secs()*1000 + (dur.subsec_nanos() / 1000000) as u64;
}

pub fn ms_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

pub fn serialize_timestamp<S>(tm: &SystemTime, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
{
    ser.serialize_u64(tstamp_to_ms(*tm))
}

pub fn deserialize_timestamp<'de, D>(de: D) -> Result<SystemTime, D::Error>
    where D: Deserializer<'de>
{
    u64::deserialize(de).map(ms_to_system_time)
}

pub fn serialize_instant<S>(tm: &Instant, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
{
    ser.serialize_u64(instant_to_ms(*tm))
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

pub fn serialize_opt_instant<S>(tm: &Option<Instant>, ser: S)
    -> Result<S::Ok, S::Error>
    where S: Serializer
{
    match *tm {
        Some(ts) => {
            ser.serialize_some(&instant_to_ms(ts))
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
