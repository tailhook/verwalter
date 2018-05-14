use std::collections::BTreeMap;
use std::sync::Arc;

use failure::{Error};
use serde_json::Value as Json;
use scheduler::{Schedule};
use query::Settings;


pub struct Responder {
    schedule: Arc<Schedule>,
}

impl Responder {
    pub fn new(schedule: &Arc<Schedule>, _settings: &Settings) -> Responder {
        Responder {
            schedule: schedule.clone(),
        }
    }
    pub fn render_roles(&self) -> Result<BTreeMap<String, Json>, Error> {
        unimplemented!();
    }
    pub fn schedule(&self) -> Arc<Schedule> {
        self.schedule.clone()
    }
}
