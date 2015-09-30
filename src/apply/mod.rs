use std::io;
use std::collections::HashMap;

use tempfile::NamedTempFile;

use config::Config;
use render::Error as RenderError;

mod root_command;

pub type ApplyTask = HashMap<String,
    Result<Vec<(String, Action, Source)>, RenderError>>;

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
        Command(runner: String, cmd: String, err: io::Error)
    }
}

pub fn apply_list(name: &String,
    task: Result<Vec<(String, Action, Source)>, RenderError>,
    dry_run: bool)
    -> Vec<Error>
{
    use self::Action::*;
    let mut errors = Vec::new();
    match task {
        Ok(actions) => {
            for (name, cmd, source) in actions {
                match cmd {
                    RootCommand(cmd) => {
                        root_command::execute(cmd, source)
                        .map_err(|e| errors.push(e)).ok();
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

pub fn apply_all(cfg: &Config, task: ApplyTask, dry_run: bool)
    -> HashMap<String, Vec<Error>>
{
    task.into_iter().map(|(name, items)| {
        let apply_result = apply_list(&name, items, dry_run);
        (name, apply_result)
    }).collect()
}
