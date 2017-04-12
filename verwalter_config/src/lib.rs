extern crate yaml_rust;
extern crate scan_dir;
extern crate quire;
extern crate rustc_serialize;
#[macro_use] extern crate log;
#[macro_use] extern crate quick_error;

use std::io;
use std::num::ParseFloatError;
use std::path::{Path, PathBuf};

use rustc_serialize::json::Json;
use yaml_rust::Yaml;
use rustc_serialize::json::BuilderError as JsonError;
use yaml_rust::scanner::ScanError as YamlError;

mod meta;
mod tojson;
mod sandbox;

pub use self::sandbox::Sandbox;

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

impl MetadataError {
    pub fn kind(&self) -> &'static str {
        use self::MetadataError::*;
        match *self {
            ScanDir(..) => "ScanDir",
            FileRead(..) => "FileRead",
            JsonParse(..) => "JsonParse",
            YamlParse(..) => "YamlParse",
            Float(..) => "Float",
            BadYamlKey(..) => "BadYamlKey",
            BadYamlValue(..) => "BadYamlValue",
        }
    }
    pub fn path_str(&self) -> String {
        use self::MetadataError::*;
        use scan_dir::Error::{Io, Decode};
        let path: &Path = match *self {
            ScanDir(Io(_, ref p)) => p,
            ScanDir(Decode(ref p)) => p,
            FileRead(_, ref p) => p,
            JsonParse(_, ref p) => p,
            YamlParse(_, ref p) => p,
            Float(_, ref p) => p,
            BadYamlKey(_, ref p) => p,
            BadYamlValue(_, ref p) => p,
        };
        return path.display().to_string();
    }
}


#[derive(Debug)]
pub struct Runtime {
    pub data: Json,
    pub errors: Vec<MetadataError>,
}

pub fn read_runtime(dir: &Path) -> Runtime
{
    let (data, err) = meta::read_dir(dir);
    Runtime {
        data: data,
        errors: err,
    }
}
