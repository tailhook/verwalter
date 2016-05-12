use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::path::Path;
use std::collections::BTreeMap;

use rustc_serialize::json::Json;
use yaml_rust::{Yaml, YamlLoader};
use scan_dir;

use super::MetadataError;


fn convert_yaml(yaml: Yaml, path: &Path) -> Result<Json, MetadataError> {
    use super::MetadataError::{Float, BadYamlKey, BadYamlValue};
    use yaml_rust::Yaml as Y;
    use rustc_serialize::json::Json as J;
    let json = match yaml {
        Y::Real(x) => J::F64(try!(FromStr::from_str(&x)
                       .map_err(|e| Float(e, path.to_path_buf())))),
        Y::Integer(x) => J::I64(x),
        Y::String(x) => J::String(x),
        Y::Boolean(x) => J::Boolean(x),
        Y::Array(a) => {
            let mut r = vec![];
            for x in a.into_iter() {
                r.push(try!(convert_yaml(x, path)));
            }
            J::Array(r)
        }
        Y::Hash(m) => {
            let mut r = BTreeMap::new();
            for (k, v) in m.into_iter() {
                let k = match k {
                    Y::String(x) => x,
                    Y::Real(x) => x,
                    Y::Integer(x) => format!("{}", x),
                    Y::Boolean(x) => format!("{}", x),
                    e => return Err(BadYamlKey(e, path.to_path_buf())),
                };
                r.insert(k, try!(convert_yaml(v, path)));
            }
            J::Object(r)
        }
        Y::Null => J::Null,
        v @ Y::Alias(_) | v @ Y::BadValue => {
            return Err(BadYamlValue(v, path.to_path_buf()))
        }
    };
    Ok(json)
}

fn read_entry(path: &Path, ext: &str)
    -> Result<Option<Json>, MetadataError>
{
    use super::MetadataError::{FileRead, JsonParse, YamlParse};
    let value = match ext {
        "yaml" | "yml" => {
            debug!("Reading YAML metadata from {:?}", path);
            let mut buf = String::with_capacity(1024);
            try!(File::open(path)
                .and_then(|mut f| f.read_to_string(&mut buf))
                .map_err(|e| FileRead(e, path.to_path_buf())));
            let mut yaml = try!(YamlLoader::load_from_str(&buf)
                .map_err(|e| YamlParse(e, path.to_path_buf())));
            if yaml.len() < 1 {
                Some(Json::Null)
            } else {
                Some(try!(convert_yaml(yaml.remove(0), path)))
            }
        }
        "json" => {
            debug!("Reading JSON metadata from {:?}", path);
            let mut f = try!(File::open(path)
                .map_err(|e| FileRead(e, path.to_path_buf())));
            Some(try!(Json::from_reader(&mut f)
                .map_err(|e| JsonParse(e, path.to_path_buf()))))
        }
        "txt" => {
            debug!("Reading text metadata from {:?}", path);
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

pub fn read_dir(path: &Path) -> (Json, Vec<MetadataError>) {
    use super::MetadataError::ScanDir;

    let mut data = BTreeMap::new();
    let mut errors = vec!();
    scan_dir::ScanDir::files().read(path, |iter| {
        for (entry, _) in iter {
            let fpath = entry.path();
            let ext = fpath.extension().map(|x| x.to_str()).unwrap();
            if ext.is_none() { continue; }
            match read_entry(&entry.path(), ext.unwrap()) {
                Ok(Some(value)) => {
                    let stem = fpath.file_stem().unwrap().to_str().unwrap();
                    data.insert(stem.to_string(), value);
                }
                Ok(None) => {}
                Err(e) => {
                    errors.push(e);
                }
            }
        }
    }).map_err(|e| errors.push(ScanDir(e))).ok();
    return (Json::Object(data), errors);
}
