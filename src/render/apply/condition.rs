use std::path::PathBuf;
use std::collections::HashMap;
use std::io::Cursor;

use hex::encode as to_hex;
use failure::ResultExt;
use dir_signature::{ScannerConfig, HashType, v1, get_hash};
use quire::validate as V;

use apply::{Command, Action, Task, Variables, Error};
use renderfile::command_validator;


#[derive(Deserialize, Debug, Clone)]
pub struct Condition {
    dirs_changed: Vec<PathBuf>,
    commands: Vec<Command>,
}

pub struct Data {
    dirs: HashMap<PathBuf, String>,
}

impl Condition {
    pub fn config() -> V::Structure<'static> {
        V::Structure::new()
        .member("dirs_changed", V::Sequence::new(V::Scalar::new()))
        .member("commands", V::Sequence::new(command_validator(false)))
    }
}

impl Action for Condition {
    fn needs_pitch(&self) -> bool {
        self.dirs_changed.len() > 0
    }

    fn pitch(&self, task: &mut Task, _variables: &Variables)
        -> Result<(), Error>
    {
        for dir in &self.dirs_changed {
            if dir.exists() {
                let mut cfg = ScannerConfig::new();
                cfg.auto_threads();
                cfg.hash(HashType::blake2b_256());
                cfg.add_dir(&dir, "/");
                let mut index_buf = Vec::new();
                v1::scan(&cfg, &mut index_buf)
                    .context(dir.display().to_string())?;
                task.scratch.condition.dirs.insert(dir.clone(),
                    to_hex(get_hash(&mut Cursor::new(&index_buf))?));
            }
        }
        Ok(())
    }

    fn execute(&self, task: &mut Task, variables: &Variables)
        -> Result<(), Error>
    {
        let mut changed = false;
        for dir in &self.dirs_changed {
            let old_hash = task.scratch.condition.dirs.get(dir);
            changed = match (dir.exists(), old_hash) {
                (false, None) => {
                    task.log.log(format_args!(
                        "Condition: {:?} not exists", dir));
                    false
                }
                (true, Some(old_hash)) => {
                    let mut cfg = ScannerConfig::new();
                    cfg.auto_threads();
                    cfg.hash(HashType::blake2b_256());
                    cfg.add_dir(&dir, "/");
                    let mut index_buf = Vec::new();
                    v1::scan(&cfg, &mut index_buf)
                        .context(dir.display().to_string())?;
                    let hash = to_hex(get_hash(&mut Cursor::new(&index_buf))?);
                    if &hash != old_hash {
                        task.log.log(format_args!(
                            "Condition: {:?} changed {:.6} -> {:.6}\n",
                            dir, old_hash, hash));
                        true
                    } else {
                        task.log.log(format_args!(
                            "Condition: {:?} unchanged {:.6}\n",
                            dir, old_hash));
                        false
                    }
                }
                (true, None) => {
                    task.log.log(format_args!(
                        "Condition: {:?} new directory\n", dir));
                    true
                }
                (false, Some(_)) => {
                    task.log.log(format_args!(
                        "Condition: {:?} directory deleted\n", dir));
                    true
                }
            };
            if changed {
                break;
            }
        };
        if !changed {
            return Ok(());
        }
        for cmd in &self.commands {
            cmd.execute(task, variables)?;
        }
        Ok(())
    }
}

impl Data {
    pub fn new() -> Data {
        Data {
            dirs: HashMap::new(),
        }
    }
}
