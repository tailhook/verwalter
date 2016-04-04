use std::io;
use std::fmt::Arguments;
use std::path::PathBuf;
use std::time::Duration;
use std::process::ExitStatus;
use std::collections::HashMap;

use rand::{thread_rng, Rng};
use tempfile::NamedTempFile;
use rustc_serialize::json::{Json, ToJson};

use render;
use apply;
use shared::{Id, Peer, SharedState};
use config::Config;
use render::Error as RenderError;
use watchdog::{ExitOnReturn, Alarm};
use fs_util::write_file;


mod root_command;
mod expand;
pub mod log;

pub struct Settings {
    pub print_configs: bool,
    pub hostname: String,
    pub dry_run: bool,
    pub log_dir: PathBuf,
    pub schedule_file: PathBuf,
}

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

pub fn apply_list(role: &String,
    task: Result<Vec<(String, Action, Source)>, RenderError>,
    log: &mut log::Deployment, dry_run: bool)
    -> Vec<Error>
{
    use self::Action::*;
    let mut errors = Vec::new();
    let mut role_log = match log.role(role) {
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
                        let vars = expand::Variables::new()
                           .add("role_name", role)
                           .add_source(&source);
                        root_command::execute(cmd, Task {
                            runner: &aname,
                            log: &mut action,
                            dry_run: dry_run,
                            source: source,
                        }, vars).map_err(|e| errors.push(e)).ok();
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
    let roles = task.into_iter().map(|(role, items)| {
        let apply_result = apply_list(&role, items, &mut log, dry_run);
        (role, apply_result)
    }).collect();
    let glob = log.done().into_iter().map(From::from).collect();
    (roles, glob)
}


fn apply_schedule(config: &Config, hash: &String, scheduler_result: &Json,
    peers: &HashMap<Id, Peer>, settings: &Settings)
{
    let apply_task = match render::render_all(config,
        &scheduler_result, &settings.hostname,
                            settings.print_configs)
    {
        Ok(res) => res,
        Err(e) => {
            error!("Configuration render failed: {}", e);
            return;
        }
    };
    if log_enabled!(::log::LogLevel::Debug) {
        for (role, result) in &apply_task {
            match result {
                &Ok(ref v) => {
                    debug!("Role {:?} has {} apply tasks", role, v.len());
                }
                &Err(render::Error::Skip) => {
                    debug!("Role {:?} is skipped on the node", role);
                }
                &Err(ref e) => {
                    debug!("Role {:?} has error: {}", role, e);
                }
            }
        }
    }

    let id = thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = apply::log::Index::new(
        &settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(id);
    dlog.string("schedule-hash", &hash);
    dlog.object("config", &config);
    dlog.object("peers", &peers);
    dlog.json("scheduler_result", &scheduler_result);
    let (rerrors, gerrs) = apply::apply_all(apply_task, dlog,
        settings.dry_run);
    if log_enabled!(::log::LogLevel::Debug) {
        for e in gerrs {
            error!("Error when applying config: {}", e);
        }
        for (role, errs) in rerrors {
            for e in errs {
                error!("Error when applying config for {:?}: {}", role, e);
            }
        }
    }
}

pub fn run(state: SharedState, settings: Settings, mut alarm: Alarm) -> ! {
    let _guard = ExitOnReturn(93);
    let mut prev_schedule = String::new();
    if let Some(schedule) = state.stable_schedule() {
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&*state.config(),
            &schedule.hash, &schedule.data,
            &state.peers().expect("peers").1, &settings);
        prev_schedule = schedule.hash.clone();
    }
    loop {
        let schedule = state.wait_new_schedule(&prev_schedule);
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&*state.config(),
            &schedule.hash, &schedule.data,
            &state.peers().expect("peers").1, &settings);
        prev_schedule = schedule.hash.clone();
    }
}
