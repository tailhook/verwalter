use std::sync::Arc;
use std::path::Path;

use failure::Error;
use query::Settings;
use serde_json::{Value as Json};

use scheduler::{Schedule};
use wasm::Program;
use super::RolesResult;


pub struct Responder {
    schedule: Arc<Schedule>,
    wasm: Program,
}

#[derive(Debug, Serialize)]
pub struct QueryInit<'a> {
    schedule: &'a Schedule,
    hostname: &'a str,
}

#[derive(Debug, Serialize)]
pub struct RolesQuery<'a> {
    deployment_id: &'a str,
    previous_schedule: Option<&'a Schedule>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ErrorKind {
    Serialize,
    Deserialize,
    Internal,
    Other(String)
}

#[derive(Debug, Fail, Deserialize)]
#[fail(display="query error {:?}: {}", kind, message)]
struct QueryError {
    kind: ErrorKind,
    message: String,
    causes: Option<Vec<String>>,
    backtrace: Option<String>,
}

#[derive(Debug, Fail, Deserialize)]
#[serde(untagged)]
enum CatchAllError {
    #[fail(display="{}", _0)]
    Known(QueryError),
    #[fail(display="unknown error: {:?}", _0)]
    Unknown(Json),
}

impl Responder {
    pub fn new(schedule: &Arc<Schedule>, settings: &Settings,
               file: &Path)
        -> Result<Responder, Error>
    {
        let mut wasm = Program::read(file)?;
        let init_res: Result<(), CatchAllError> = wasm.json_call("init",
            &QueryInit {
                schedule: &*schedule,
                hostname: &settings.hostname,
            })?;
        init_res?;
        Ok(Responder {
            schedule: schedule.clone(),
            wasm,
        })
    }

    pub fn render_roles(&mut self, id: &str, prev: Option<&Schedule>)
        -> Result<RolesResult, Error>
    {
        let result: Result<_, CatchAllError>;
        result = self.wasm.json_call("render_roles", &RolesQuery {
            deployment_id: id,
            previous_schedule: prev,
        })?;
        return Ok(result?);
    }

    pub fn schedule(&self) -> Arc<Schedule> {
        self.schedule.clone()
    }
}

mod serde {
    use serde::de::{Deserialize, Deserializer};
    use super::ErrorKind;

    impl<'de> Deserialize<'de> for ErrorKind {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: Deserializer<'de>
        {
            use super::ErrorKind::*;

            let s = String::deserialize(deserializer)?;
            Ok(match s.as_str() {
                "Serialize" => Serialize,
                "Deserialize" => Deserialize,
                "Internal" => Internal,
                _ => Other(s),
            })
        }
    }
}
