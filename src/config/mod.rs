use std::io;
use std::path::PathBuf;
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


pub struct MetadataErrors {
    pub errors: Vec<MetadataError>,
    pub partial: Json,
}

pub struct TemplateErrors {
    pub errors: Vec<TemplateError>,
}

pub struct Config {
    pub machine: Result<Json, MetadataErrors>,
    pub roles: HashMap<String, Role>,
}

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

pub fn read_configs(options: &Options, cache: &mut Cache) -> Config {
    let mut cfg = Config {
        machine: Ok(Json::Object(BTreeMap::new())),
        roles: HashMap::new(),
    };
    cfg
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
