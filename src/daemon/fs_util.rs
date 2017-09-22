use std::io::{self, Write, Read, BufWriter};
use std::path::Path;
use std::io::ErrorKind::{NotFound, AlreadyExists};
use std::fs::{File, OpenOptions, remove_file, create_dir, rename, metadata};

use rustc_serialize::{Encodable};
use rustc_serialize::json::{as_pretty_json, Json};


pub fn ensure_dir(dir: &Path) -> Result<(), io::Error> {
    // TODO(tailhook) check if thing is a directory
    match metadata(&dir) {
        Ok(_) => Ok(()),
        Err(ref e) if e.kind() == NotFound => {
            match create_dir(&dir) {
                Ok(()) => Ok(()),
                Err(ref e) if e.kind() == AlreadyExists => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        Err(e) => return Err(e),
    }
}



pub fn read_json(path: &Path) -> io::Result<Json> {
    let mut buf = String::with_capacity(4096);
    let mut file = try!(File::open(path));
    try!(file.read_to_string(&mut buf));
    Json::from_str(&buf).map_err(
        |_| io::Error::new(io::ErrorKind::InvalidData, "Can't decode json"))
}

pub fn safe_write(path: &Path, data: &[u8]) -> io::Result<()> {
    ensure_dir(&path.parent().expect("valid dir"))?;
    let tmppath = path.with_extension("tmp");
    match remove_file(&tmppath) {
        Ok(()) => {}
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut file = OpenOptions::new().create_new(true).write(true)
        .open(&tmppath)?;
    file.write_all(data)?;
    rename(&tmppath, path)?;
    Ok(())
}

pub fn write_file<T: Encodable>(path: &Path, data: &T) -> io::Result<()> {
    try!(ensure_dir(&path.parent().expect("valid dir")));
    let tmppath = path.with_extension("tmp");
    let mut file = BufWriter::new(try!(File::create(&tmppath)));
    try!(write!(&mut file, "{}", as_pretty_json(data)));
    try!(rename(&tmppath, path));
    Ok(())
}
