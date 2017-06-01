use std::net::SocketAddr;
use std::time::SystemTime;

use rustc_serialize::json::{Json, ToJson};


#[derive(Clone, Debug)]
pub struct Peer {
     pub addr: Option<SocketAddr>,
     pub name: String,
     pub hostname: String,
     // pub addressses: Vec<SocketAddr>,  // TODO(tailhook)
     // pub known_since: SystemTime,  // TODO(tailhook)
     // pub last_report_direct: Option<SystemTime>,  // TODO(tailhook)
}

impl ToJson for Peer {
    fn to_json(&self) -> Json {
        Json::Object(vec![
            ("hostname".to_string(), self.hostname.to_json()),
        ].into_iter().collect())
    }
}
