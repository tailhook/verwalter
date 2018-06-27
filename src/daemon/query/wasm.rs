use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::Path;

use failure::{Error, err_msg};
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
        let init_res: Result<(), String> = wasm.json_call("init", &QueryInit {
            schedule: &*schedule,
            hostname: &settings.hostname,
        })?;
        init_res.map_err(|e| err_msg(e))?;
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
