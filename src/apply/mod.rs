use std::io;
use std::fmt::{Arguments, Debug};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::process::ExitStatus;
use std::collections::HashMap;

use rand::{thread_rng, Rng};
use tempfile::NamedTempFile;
use rustc_serialize::{Decodable, Decoder};
use rustc_serialize::json::{Json, ToJson};

use render;
use apply;
use shared::{Id, Peer, SharedState};
use config::Config;
use render::Error as RenderError;
use watchdog::{ExitOnReturn, Alarm};
use fs_util::write_file;
use apply::expand::Variables;

pub mod root_command;
pub mod cmd;
pub mod shell;
pub mod copy;
mod expand;
pub mod log;

const COMMANDS: &'static [&'static str] = &[
    "RootCommand",
    "Cmd",
    "Sh",
    "Copy",
];

pub struct Settings {
    pub print_configs: bool,
    pub hostname: String,
    pub dry_run: bool,
    pub log_dir: PathBuf,
    pub schedule_file: PathBuf,
}

pub type ApplyTask = HashMap<String,
    Result<Vec<(String, Command, Source)>, RenderError>>;

pub struct Task<'a: 'b, 'b: 'c, 'c: 'd, 'd> {
    pub runner: &'d str,
    pub log: &'d mut log::Action<'a, 'b, 'c>,
    pub dry_run: bool,
    pub source: Source,
}

trait Action: Debug + Send + ToJson + Sync {
    fn execute(&self, task: Task, variables: Variables)
        -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub struct Command(Arc<Action>);

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
        InvalidArgument(message: &'static str, value: String) {
            display("{}: {:?}", message, value)
            description(message)
        }
        IoError(err: io::Error) {
            from() cause(err)
            display("io error: {}", err)
            description(err.description())
        }
    }
}

fn cmd<T: Action + 'static, E>(val: Result<T, E>)
    -> Result<Command, E>
{
    val.map(|x| Command(Arc::new(x) as Arc<Action>))
}

fn decode_command<D: Decoder>(cmdname: &str, d: &mut D)
    -> Result<Command, D::Error>
{
    match cmdname {
        "RootCommand" => cmd(self::root_command::RootCommand::decode(d)),
        "Cmd" => cmd(self::cmd::Cmd::decode(d)),
        "Sh" => cmd(self::shell::Sh::decode(d)),
        "Copy" => cmd(self::copy::Copy::decode(d)),
        _ => panic!("Command {:?} not implemented", cmdname),
    }
}

impl Decodable for Command {
    fn decode<D: Decoder>(d: &mut D) -> Result<Command, D::Error> {
        Ok(try!(d.read_enum("Action", |d| {
            d.read_enum_variant(&COMMANDS, |d, index| {
                decode_command(COMMANDS[index], d)
            })
        })))
    }
}

impl ToJson for Command {
    fn to_json(&self) -> Json {
        self.0.to_json()
    }
}

impl Action for Command {
    fn execute(&self, task: Task, variables: Variables)
        -> Result<(), Error>
    {
        self.0.execute(task, variables)
    }
}


impl<'a, 'b, 'c, 'd> Task<'a, 'b, 'c, 'd> {
    fn log(&mut self, args: Arguments) {
        if self.dry_run {
            self.log.log(format_args!("(dry_run) {}\n", args));
        } else {
            self.log.log(args);
        }
    }
}

pub fn apply_list(role: &String,
    actions: Vec<(String, Command, Source)>,
    log: &mut log::Role, dry_run: bool)
{
    for (aname, cmd, source) in actions {
        let mut action = log.action(&aname);
        let vars = expand::Variables::new()
           .add("role_name", role)
           .add_source(&source);
        cmd.execute(Task {
            runner: &aname,
            log: &mut action,
            dry_run: dry_run,
            source: source,
        }, vars).map_err(|e| action.error(&e)).ok();
    }
}

fn apply_schedule(config: &Config, hash: &String, scheduler_result: &Json,
    peers: &HashMap<Id, Peer>, debug: Option<Arc<String>>, settings: &Settings)
{
    let id = thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = apply::log::Index::new(
        &settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(id);
    dlog.string("schedule-hash", &hash);
    dlog.object("config", &config);
    dlog.object("peers", &peers);
    if let Some(debug) = debug {
        dlog.text("debug_info", &*debug);
    }
    dlog.json("scheduler_result", &scheduler_result);

    let meta = scheduler_result.as_object()
        .and_then(|x| x.get("role_metadata"))
        .and_then(|y| y.as_object());
    let meta = match meta {
        Some(meta) => meta,
        None => {
            dlog.log(format_args!(
                "FATAL ERROR: Can't find `role_metadata` key in schedule\n"));
            error!("Can't find `role_metadata` key in schedule");
            return
        }
    };
    let node = scheduler_result.as_object()
        .and_then(|x| x.get("nodes"))
        .and_then(|y| y.as_object())
        .and_then(|x| x.get(&settings.hostname))
        .and_then(|y| y.as_object());
    let node = match node {
        Some(node) => node,
        None => {
            dlog.log(format_args!(
                "FATAL ERROR: Can't find node {:?} in `nodes` \
                    key in schedule\n",
                &settings.hostname));
            error!("Can't find node {:?} in `nodes` key in schedule",
                &settings.hostname);
            return;
        }
    };

    for (role_name, role) in &config.roles {
        let mut rlog = match dlog.role(&role_name) {
            Ok(l) => l,
            Err(e) => {
                error!("Can't create role log: {}", e);
                return;
            }
        };

        match render::render_role(meta, node, &role_name, &role, &mut rlog) {
            Ok(actions) => {
                apply_list(&role_name, actions, &mut rlog, settings.dry_run);
            }
            Err(e) => {
                rlog.log(format_args!(
                    "ERROR: Can't render templates: {}\n", e));
                error!("Can't render templates for role {:?}: {}",
                    role_name, e);
            }
        }
    }
    for err in dlog.done() {
        error!("Error when doing deployment logging: {}", err);
    }
}

pub fn run(state: SharedState, settings: Settings, mut alarm: Alarm) -> ! {
    let _guard = ExitOnReturn(93);
    let mut prev_schedule = String::new();
    if let Some((schedule, debug)) = state.schedule_and_debug_info() {
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&*state.config(),
            &schedule.hash, &schedule.data,
            &state.peers().expect("peers").1, debug, &settings);
        prev_schedule = schedule.hash.clone();
    }
    loop {
        let (schedule, debug) = state.wait_new_schedule(&prev_schedule);
        let _alarm = alarm.after(Duration::from_secs(180));
        write_file(&settings.schedule_file, &*schedule)
            .map(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&*state.config(),
            &schedule.hash, &schedule.data,
            &state.peers().expect("peers").1, debug, &settings);
        prev_schedule = schedule.hash.clone();
    }
}
