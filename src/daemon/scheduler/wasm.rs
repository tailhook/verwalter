use std::path::{Path};
use failure::{Error, err_msg};
use serde_json::{Value as Json};
use serde::Serialize;


#[derive(Debug, Fail)]
#[allow(dead_code)]
pub enum ReadError {
    #[fail(display="Fatal error when reading file: {:?}", _0)]
    Fatal(Error),
}

pub(in scheduler) struct Scheduler {
}

pub(in scheduler) fn read(_dir: &Path)
    -> Result<Scheduler, ReadError>
{
    Ok(Scheduler {
    })
}

impl Scheduler {
    pub fn execute<S: Serialize>(&mut self, _input: &S)
        -> (Result<Json, Error>, String)
    {
        (Err(err_msg("wasm scheduler is unimplemented")), String::from(""))
    }
}
