use std::io;
use std::fmt::Arguments;
use std::process::ExitStatus;
use std::collections::HashMap;

use tempfile::NamedTempFile;
use rustc_serialize::json::{Json, ToJson};

use render::Error as RenderError;


mod root_command;
pub mod log;


pub type ApplyTask = HashMap<String,
    Result<Vec<(String, Action, Source)>, RenderError>>;

pub struct Task<'a: 'b, 'b: 'c, 'c: 'd, 'd> {
    pub runner: &'d str,
    pub log: &'d mut log::Action<'a, 'b, 'c>,
    pub dry_run: bool,
    pub source: Source,
}

#[derive(RustcDecodable, Debug, Clone)]
pub enum Action {
    RootCommand(Vec<String>),
}

pub enum Source {
    TmpFile(NamedTempFile),
}

quick_error!{
    #[derive(Debug)]
    pub enum Error {
        Command(runner: String, cmd: String, status: ExitStatus) {
            display("Action {:?} failed to run {:?}: {}", runner, cmd, status)
            description("error running command")
        }
        CantRun(runner: String, cmd: String, err: io::Error) {
            display("Action {:?} failed to run {:?}: {}", runner, cmd, err)
            description("error running command")
        }
        Log(err: log::Error) {
            from() cause(err)
            display("error opening log file: {}", err)
            description("error logging command info")
        }
    }
}

impl ToJson for Action {
    fn to_json(&self) -> Json {
        match *self {
            Action::RootCommand(ref cmd) => {
                Json::Array(vec!["RootCommand".to_json(), cmd.to_json()])
            }
        }
    }
}

impl<'a, 'b, 'c, 'd> Task<'a, 'b, 'c, 'd> {
    fn log(&mut self, args: Arguments) {
        if self.dry_run {
            self.log.log(format_args!("(dry_run) {}", args));
        } else {
            self.log.log(args);
        }
    }
}

pub fn apply_list(name: &String,
    task: Result<Vec<(String, Action, Source)>, RenderError>,
    log: &mut log::Deployment, dry_run: bool)
    -> Vec<Error>
{
    use self::Action::*;
    let mut errors = Vec::new();
    let mut role_log = match log.role(name) {
        Ok(l) => l,
        Err(e) => {
            errors.push(From::from(e));
            return errors;
        }
    };
    match task {
        Ok(actions) => {
            for (aname, cmd, source) in actions {
                let mut action = role_log.action(&aname);
                match cmd {
                    RootCommand(cmd) => {
                        root_command::execute(cmd, Task {
                            runner: &aname,
                            log: &mut action,
                            dry_run: dry_run,
                            source: source,
                        }).map_err(|e| errors.push(e)).ok();
                    }
                }
            }
        }
        Err(_) => {
            // TODO(tailhook) log error
            unimplemented!();
        }
    }
    return errors;
}

pub fn apply_all(task: ApplyTask, mut log: log::Deployment, dry_run: bool)
    -> (HashMap<String, Vec<Error>>, Vec<Error>)
{
    let roles = task.into_iter().map(|(name, items)| {
        let apply_result = apply_list(&name, items, &mut log, dry_run);
        (name, apply_result)
    }).collect();
    let glob = log.done().into_iter().map(From::from).collect();
    (roles, glob)
}
