use std::io;
use std::io::Read;
use std::num::ParseFloatError;
use std::path::{PathBuf};
use std::collections::HashMap;

use rustc_serialize::json::Json;
use handlebars::TemplateError as HandlebarsError;
use yaml_rust::Yaml;
use rustc_serialize::json::BuilderError as JsonError;
use yaml_rust::scanner::ScanError as YamlError;
use scan_dir;

use super::render::RenderSet;
use self::render::read_renderers;
use Options;

pub use self::version::Version;
//pub use self::template::Template;

mod meta;
//mod template;
mod version;
mod render;

quick_error! {
    #[derive(Debug)]
    pub enum MetadataError {
        ScanDir(err: scan_dir::Error) {
            cause(err)
            display("{}", err)
            description("error reading configuration directory")
        }
        FileRead(err: io::Error, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading configuration file")
        }
        JsonParse(err: JsonError, path: PathBuf) {
            cause(err)
            display("error parsing json {:?}: {}", path, err)
            description("error parsing json metadata")
        }
        YamlParse(err: YamlError, path: PathBuf) {
            cause(err)
            display("error parsing yaml {:?}: {}", path, err)
            description("error parsing yaml metadata")
        }
        Float(err: ParseFloatError, path: PathBuf) {
            cause(err)
            display("error parsing float in {:?}: {}", path, err)
        }
        /// Some valid yaml keys can't be json keys
        BadYamlKey(key: Yaml, path: PathBuf) {
            display("bad key in yaml {:?}, key: {:?}", path, key)
        }
        /// Some valid yaml keys does not work in json
        BadYamlValue(key: Yaml, path: PathBuf) {
            display("bad value in yaml {:?}, key: {:?}", path, key)
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
        TemplateParse(err: HandlebarsError, path: PathBuf) {
            cause(err)
            display("error reading {:?}: {}", path, err)
            description("error reading template file")
        }
        Config(err: String, path: PathBuf) {
            display("error reading {:?}: {}", path, err)
            description("error reading config from template dir")
        }
        ScanDir(err: scan_dir::Error) {
            from() cause(err)
            display("{}", err)
            description("error reading template directory")
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

pub struct Cache;

impl Cache {
    pub fn new() -> Cache {
        Cache
    }
}

pub fn read_configs(options: &Options, _cache: &mut Cache)
    -> Result<Config, scan_dir::Error>
{
    let meta = meta::read_dir(&options.config_dir.join("machine"));
    let mut roles = HashMap::new();

    let tpldir = options.config_dir.join("templates");
    try!(scan_dir::ScanDir::dirs().read(tpldir, |iter| {
        for (entry, name) in iter {
            let mut role = Role {
                renderers: HashMap::new(),
                runtime: HashMap::new(),
            };
            try!(scan_dir::ScanDir::dirs().read(entry.path(), |iter| {
                for (entry, version) in iter {
                    debug!("Reading template version {:?} of {:?}",
                        version, name);
                    role.renderers.insert(Version(version),
                        read_renderers(&entry.path()));
                }
            }));
            roles.insert(name.to_string(), role);
        }
        Ok(())
    }).and_then(|x| x));

    let tpldir = options.config_dir.join("runtime");
    try!(scan_dir::ScanDir::dirs().read(tpldir, |iter| {
        for (entry, name) in iter {
            if let Some(ref mut role) = roles.get_mut(&name) {
                try!(scan_dir::ScanDir::dirs().read(entry.path(), |iter| {
                    for (entry, version) in iter {
                        debug!("Reading runtime version {:?} of {:?}",
                            version, name);
                        role.runtime.insert(Version(version),
                            meta::read_dir(&entry.path()));
                    }
                }));
            } else {
                warn!("Ignored runtime data for {:?} \
                    because to templates found", name);
            }
        }
        Ok(())
    }).and_then(|x| x));

    Ok(Config {
        machine: meta,
        roles: roles,
    })
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
