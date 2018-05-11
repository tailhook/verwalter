use std::sync::Arc;

use scheduler::{Schedule};
use query::Settings;


pub struct Responder {

}

impl Responder {
    pub fn new(schedule: &Arc<Schedule>, _settings: &Settings) -> Responder {
        unimplemented!();
    }
}
