use std::net::SocketAddr;
use std::time::SystemTime;

use rustc_serialize::json::{Json, ToJson};


#[derive(Clone, Debug, Serialize)]
pub struct Peer {
     pub addr: Option<SocketAddr>,
     pub name: String,
     pub hostname: String,
     // pub addressses: Vec<SocketAddr>,  // TODO(tailhook)
     // pub known_since: SystemTime,  // TODO(tailhook)
     // pub last_report_direct: Option<SystemTime>,  // TODO(tailhook)
}
