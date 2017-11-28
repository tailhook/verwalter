use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::path::Path;
use std::path::Component::Normal;

use serde_json::{self, Value, Number, Map};
use yaml_rust::{Yaml, YamlLoader};
use scan_dir;

use super::MetadataError;


fn convert_yaml(yaml: Yaml, path: &Path) -> Result<Value, MetadataError> {
    use super::MetadataError::{Float, BadYamlKey, BadYamlValue, NanOrInfinity};
    use yaml_rust::Yaml as Y;
    use serde_json::Value as V;
    let json = match yaml {
        Y::Real(x) => V::Number(Number::from_f64(f64::from_str(&x)
                       .map_err(|e| Float(e, path.to_path_buf()))?)
                       .ok_or(NanOrInfinity(path.to_path_buf()))?),
        Y::Integer(x) => V::Number(x.into()),
        Y::String(x) => V::String(x),
        Y::Boolean(x) => V::Bool(x),
        Y::Array(a) => {
            let mut r = vec![];
            for x in a.into_iter() {
                r.push(try!(convert_yaml(x, path)));
            }
            V::Array(r)
        }
        Y::Hash(m) => {
            let mut r = Map::new();
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
            V::Object(r)
        }
        Y::Null => V::Null,
        v @ Y::Alias(_) | v @ Y::BadValue => {
            return Err(BadYamlValue(v, path.to_path_buf()))
        }
    };
    Ok(json)
}

fn read_entry(path: &Path, ext: &str)
    -> Result<Option<Value>, MetadataError>
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
                Some(Value::Null)
            } else {
                Some(try!(convert_yaml(yaml.remove(0), path)))
            }
        }
        "json" => {
            debug!("Reading JSON metadata from {:?}", path);
            let mut f = try!(File::open(path)
                .map_err(|e| FileRead(e, path.to_path_buf())));
            Some(try!(serde_json::from_reader(&mut f)
                .map_err(|e| JsonParse(e, path.to_path_buf()))))
        }
        "txt" => {
            debug!("Reading text metadata from {:?}", path);
            let mut buf = String::with_capacity(100);
            try!(File::open(path)
                .and_then(|mut f| f.read_to_string(&mut buf))
                .map_err(|e| FileRead(e, path.to_path_buf())));
            Some(Value::String(buf))
        }
        _ => None,
    };
    Ok(value)
}

pub fn read_dir(path: &Path) -> (Value, Vec<MetadataError>) {
    use super::MetadataError::ScanDir;

    let mut data = Value::Object(Map::new());
    let mut errors = vec!();
    scan_dir::ScanDir::files().walk(path, |iter| {
        for (entry, _) in iter {
            let fpath = entry.path();
            let ext = fpath.extension().and_then(|x| x.to_str());
            if ext.is_none() { continue; }
            let value = match read_entry(&fpath, ext.unwrap()) {
                Ok(Some(value)) => value,
                Ok(None) => continue,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let ptr = fpath.strip_prefix(path).unwrap()
                .components()
                .filter_map(|x| match x {
                    Normal(p) => Some(p),
                    _ => None,
                })
                .map(|pstr| {
                    let p = Path::new(pstr);
                    match p.extension().and_then(|x| x.to_str()).unwrap_or("")
                    {
                        "yaml" | "yml" | "json" | "txt" | "d" => {
                            p.file_stem().and_then(|x| x.to_str()).unwrap()
                        }
                        _ => pstr.to_str().unwrap(),
                    }
                })
                .fold(&mut data, |map, key| {
                    if !map.is_object() {
                        *map = Value::Object(Map::new())
                    };
                    map.as_object_mut().unwrap().entry(key.to_string())
                    .or_insert_with(|| {
                        Value::Object(Map::new())
                    })
                });
            *ptr = value;
        }
    }).map_err(|e| errors.extend(e.into_iter().map(ScanDir))).ok();
    return (data, errors);
}
