use std::io;
use std::fmt::{self, Arguments, Debug};
use std::sync::Arc;
use std::process::ExitStatus;
use std::collections::HashMap;

use serde::de::{Deserializer, Deserialize, Error as DeError, Visitor};
use serde::de::{VariantAccess, EnumAccess};
use tempfile::NamedTempFile;
use indexed_log as log;

use config::Sandbox;
use apply::expand::Variables;

mod expand;

// commands
pub mod root_command;
pub mod cmd;
pub mod shell;
pub mod copy;
pub mod peek_log;
pub mod split_text;

const COMMANDS: &'static [&'static str] = &[
    "RootCommand",
    "Cmd",
    "Sh",
    "Copy",
    "SplitText",
    "PeekLog",
];

pub enum CommandName {
    RootCommand,
    Cmd,
    Sh,
    Copy,
    SplitText,
    PeekLog,
}

pub struct NameVisitor;
pub struct CommandVisitor;

pub struct Task<'a: 'b, 'b: 'c, 'c: 'd, 'd> {
    pub runner: &'d str,
    pub log: &'d mut log::Action<'a, 'b, 'c>,
    pub dry_run: bool,
    pub source: &'d Source,
    pub sandbox: &'d Sandbox,
}

trait Action: Debug + Send + Sync {
    fn execute(&self, task: Task, variables: Variables)
        -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub struct Command(Arc<Action>);

pub enum Source {
    TmpFiles(HashMap<String, NamedTempFile>),
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
            display("{}: {}", message, value)
            description(message)
        }
        FormatError(message: String) {
            display("{}", message)
        }
        Other(message: String) {
            display("{}", message)
        }
        IoError(err: io::Error) {
            from() cause(err)
            display("io error: {}", err)
            description(err.description())
        }
    }
}

impl<'a> Deserialize<'a> for CommandName {
    fn deserialize<D: Deserializer<'a>>(d: D) -> Result<CommandName, D::Error>
    {
        d.deserialize_identifier(NameVisitor)
    }
}

impl<'a> Visitor<'a> for NameVisitor {
    type Value = CommandName;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "command is one of {}", COMMANDS.join(", "))
    }
    fn visit_str<E: DeError>(self, val: &str) -> Result<CommandName, E> {
        use self::CommandName::*;
        let res = match val {
            "RootCommand" => RootCommand,
            "Cmd" => Cmd,
            "Sh" => Sh,
            "Copy" => Copy,
            "SplitText" => SplitText,
            "PeekLog" => PeekLog,
            _ => return Err(E::custom("invalid command")),
        };
        Ok(res)
    }
}

fn decode<'x, T, V>(v: V)
    -> Result<Command, V::Error>
    where
        T: Action + Deserialize<'x> + 'static,
        V: VariantAccess<'x>,
{
    v.newtype_variant::<T>().map(|x| Command(Arc::new(x) as Arc<Action>))
}

impl<'a> Visitor<'a> for CommandVisitor {
    type Value = Command;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "command is one of {}", COMMANDS.join(", "))
    }
    fn visit_enum<A>(self, data: A) -> Result<Command, A::Error>
        where A: EnumAccess<'a>,
    {
        use self::CommandName::*;
        let (tag, v) = data.variant()?;
        match tag {
            RootCommand => decode::<root_command::RootCommand, _>(v),
            Cmd => decode::<cmd::Cmd, _>(v),
            Sh => decode::<shell::Sh, _>(v),
            Copy => decode::<copy::Copy, _>(v),
            SplitText => decode::<split_text::SplitText, _>(v),
            PeekLog => decode::<peek_log::PeekLog, _>(v),
        }
    }
}

impl<'a> Deserialize<'a> for Command {
    fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Command, D::Error> {
        d.deserialize_enum("Command", COMMANDS, CommandVisitor)
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
    actions: Vec<(String, Vec<Command>, Source)>,
    log: &mut log::Role, dry_run: bool,
    sandbox: &Sandbox)
    -> Result<(), Error>
{
    for (aname, commands, source) in actions {
        let mut action = log.action(&aname);
        for cmd in commands {
            let vars = expand::Variables::new()
               .add("role", role)
               .add_source(&source);
            try!(cmd.execute(Task {
                runner: &aname,
                log: &mut action,
                dry_run: dry_run,
                source: &source,
                sandbox: sandbox,
            }, vars));
        }
    }
    Ok(())
}
