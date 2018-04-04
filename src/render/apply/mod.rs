use std::fmt::{self, Arguments, Debug};
use std::sync::Arc;
use std::collections::HashMap;

use failure::Error;
use serde::de::{Deserializer, Deserialize, Error as DeError, Visitor};
use serde::de::{VariantAccess, EnumAccess};
use tempfile::NamedTempFile;
use indexed_log as log;

use config::Sandbox;
use apply::expand::Variables;

mod expand;

// commands
pub mod cmd;
pub mod condition;
pub mod copy;
pub mod clean_files;
pub mod peek_log;
pub mod root_command;
pub mod shell;
pub mod split_text;

const COMMANDS: &'static [&'static str] = &[
    "RootCommand",
    "Cmd",
    "Sh",
    "Copy",
    "Condition",
    "SplitText",
    "CleanFiles",
    "PeekLog",
];

pub enum CommandName {
    RootCommand,
    Cmd,
    Sh,
    Copy,
    Condition,
    SplitText,
    CleanFiles,
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
    pub scratch: Scratch,
}

// TODO(tailhook) maybe make typemap or enum?
pub struct Scratch {
    pub condition: condition::Data,
}

trait Action: Debug + Send + Sync {

    fn needs_pitch(&self) -> bool {
        false
    }

    fn pitch(&self, _task: &mut Task, _variables: &Variables)
        -> Result<(), Error>
    {
        Ok(())
    }

    fn execute(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>;
}

#[derive(Debug, Clone)]
pub struct Command(Arc<Action>);

pub enum Source {
    TmpFiles(HashMap<String, NamedTempFile>),
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
            "Condition" => Condition,
            "SplitText" => SplitText,
            "CleanFiles" => CleanFiles,
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
            Condition => decode::<condition::Condition, _>(v),
            SplitText => decode::<split_text::SplitText, _>(v),
            CleanFiles => decode::<clean_files::CleanFiles, _>(v),
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
    fn needs_pitch(&self) -> bool {
        self.0.needs_pitch()
    }

    fn pitch(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>
    {
        self.0.pitch(task, variables)
    }
    fn execute(&self, task: &mut Task, variables: &Variables)
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
    for &(ref aname, ref commands, ref source) in &actions {
        let mut action = log.action(&aname);
        let mut atasks = Vec::with_capacity(commands.len());
        for cmd in commands {
            let vars = expand::Variables::new()
               .add("role", role)
               .add_source(&source);
            if cmd.needs_pitch() {
                let mut task = Task {
                    runner: &aname,
                    log: &mut action,
                    scratch: Scratch::new(),
                    dry_run, source, sandbox,
                };
                cmd.pitch(&mut task, &vars)?;
                atasks.push((cmd, task.scratch, vars));
            } else {
                atasks.push((cmd, Scratch::new(), vars));
            }
        }
        for (cmd, scratch, vars) in atasks {
            cmd.execute(&mut Task {
                runner: &aname,
                log: &mut action,
                dry_run, source, sandbox, scratch,
            }, &vars)?;
        }
    }
    Ok(())
}

impl Scratch {
    fn new() -> Scratch {
        Scratch {
            condition: condition::Data::new(),
        }
    }
}
