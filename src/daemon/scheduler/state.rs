use serde_json::Value as Json;

use hash::hash;
use id::Id;

pub type ScheduleId = String; // temporarily

#[derive(Clone, Debug, Serialize)]
pub struct Schedule {
    pub timestamp: u64,
    pub hash: ScheduleId,
    pub data: Json,
    pub origin: Id,
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
