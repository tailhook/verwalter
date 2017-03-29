use std::net::SocketAddr;
use std::time::SystemTime;

use rustc_serialize::json::{Json, ToJson};


#[derive(Clone, Debug)]
pub struct Peer {
     pub addr: Option<SocketAddr>,
     pub name: String,
     pub hostname: String,
     pub last_report: Option<SystemTime>,
}

impl ToJson for Peer {
    fn to_json(&self) -> Json {
         unimplemented!();
         /*
            Json::Object(vec![
                ("hostname".to_string(), self.hostname.to_json()),
                ("timestamp".to_string(),
                    self.last_report.map(|x| x.to_msec()).to_json()),
            ].into_iter().collect())
        */
    }
}
