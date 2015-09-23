use std::io;
use std::io::Read;
use std::fs::{File, read_dir};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::collections::BTreeMap;

use rustc_serialize::json::Json;

use super::render::RenderSet;
use Options;

pub use self::version::Version;
pub use self::template::Template;

mod template;
mod version;

quick_error! {
    #[derive(Debug)]
    pub enum MetadataError {
        DirRead(err: io::Error, path: PathBuf) {
            cause(err)
            display("error reading dir {:?}: {}", path, err)
            description("error reading configuration directory")
        }
        FileNameDecode(path: PathBuf) {
            display("error decoding filename {:?}", path)
        }
        FileRead(err: io::Error, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading configuration file")
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum TemplateError {
        TemplateRead(err: io::Error, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
        }
    }
}


#[derive(Debug)]
pub struct MetadataErrors {
    pub errors: Vec<MetadataError>,
    pub partial: Json,
}

#[derive(Debug)]
pub struct TemplateErrors {
    pub errors: Vec<TemplateError>,
}

#[derive(Debug)]
pub struct Config {
    pub machine: Result<Json, MetadataErrors>,
    pub roles: HashMap<String, Role>,
}

#[derive(Debug)]
pub struct Role {
    // note version in template is not the same as
    pub renderers: HashMap<Version, Result<RenderSet, TemplateErrors>>,
    // ... version in runtime, role's version is here
    pub runtime: HashMap<Version, Result<Json, MetadataErrors>>,
}

pub struct Cache {
    templates: template::Cache,
}


impl Cache {
    pub fn new() -> Cache {
        Cache {
            templates: template::Cache::new(),
        }
    }
}

fn read_meta_entry(path: &Path, ext: &str)
    -> Result<Option<Json>, MetadataError>
{
    use self::MetadataError::FileRead;
    let value = match ext {
        "yaml" | "yml" => {
            unimplemented!();
        }
        "json" => {
            unimplemented!();
        }
        "txt" => {
            let mut buf = String::with_capacity(100);
            try!(File::open(path)
                .and_then(|mut f| f.read_to_string(&mut buf))
                .map_err(|e| FileRead(e, path.to_path_buf())));
            Some(Json::String(buf))
        }
        _ => None,
    };
    Ok(value)
}

fn read_meta_dir(path: &Path) -> Result<Json, MetadataErrors> {
    use self::MetadataError::{DirRead, FileNameDecode};
    let mut data = BTreeMap::new();
    let mut errors = vec!();
    match read_dir(path) {
        Ok(iter) => {
            for entryres in iter {
                let entry = match entryres {
                    Ok(entry) => entry,
                    Err(e) => {
                        errors.push(DirRead(e, path.to_path_buf()));
                        continue;
                    }
                };
                let fpath = entry.path();
                let stem = fpath.file_stem().and_then(|x| x.to_str());
                let extension = fpath.extension().and_then(|x| x.to_str());
                if let Some(stem) = stem {
                    if stem.starts_with(".") {
                        // Skip hidden files
                        continue;
                    }
                    if let Some(ext) = extension {
                        match read_meta_entry(&fpath, ext) {
                            Ok(Some(value)) => {
                                data.insert(stem.to_string(), value);
                            }
                            Ok(None) => {}
                            Err(e) => {
                                errors.push(e);
                            }
                        }
                    }
                } else {
                    // Only reason why stem is None in our case is that
                    // we can't decode filename
                    errors.push(FileNameDecode(fpath.to_path_buf()));
                }
            }
        }
        Err(e) => {
            errors.push(DirRead(e, path.to_path_buf()));
        }
    }
    if errors.len() > 0 {
        Err(MetadataErrors {
            errors: errors,
            partial: Json::Object(data),
        })
    } else {
        Ok(Json::Object(data))
    }
}

pub fn read_configs(options: &Options, cache: &mut Cache) -> Config {
    let meta = read_meta_dir(&options.config_dir.join("machine"));
    let roles = HashMap::new();
    Config {
        machine: meta,
        roles: roles,
    }
}

impl Config {
    pub fn total_errors(&self) -> usize {
        self.machine.as_ref().err().map(|x| x.errors.len()).unwrap_or(0) +
        self.roles.values().map(|r| {
            r.renderers.values()
                .map(|t| t.as_ref().err()
                          .map(|x| x.errors.len()).unwrap_or(0))
                .fold(0, |x, y| x+y) +
            r.runtime.values()
                .map(|m| m.as_ref().err()
                          .map(|x| x.errors.len()).unwrap_or(0))
                .fold(0, |x, y| x+y)
        }).fold(0, |x, y| x+y)
    }
}
