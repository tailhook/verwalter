use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::Path;

use failure::{Error};
use query::Settings;
use serde_json::{Value as Json};

use scheduler::{Schedule};
use wasm::Program;


pub struct Responder {
    schedule: Arc<Schedule>,
    wasm: Program,
}

#[derive(Debug, Serialize)]
pub struct QueryInit<'a> {
    schedule: &'a Schedule,
    hostname: &'a str,
}

impl Responder {
    pub fn new(schedule: &Arc<Schedule>, settings: &Settings,
               file: &Path)
        -> Result<Responder, Error>
    {
        let mut wasm = Program::read(file)?;
        let _ = wasm.json_call("init", &QueryInit {
            schedule: &*schedule,
            hostname: &settings.hostname,
        })?;
        Ok(Responder {
            schedule: schedule.clone(),
            wasm,
        })
    }

    pub fn render_roles(&self, id: &str)
        -> Result<BTreeMap<String, Json>, Error>
    {
        unimplemented!();
    }

    pub fn schedule(&self) -> Arc<Schedule> {
        self.schedule.clone()
    }
}
