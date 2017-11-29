use std::io::{self, Write, BufWriter, BufReader};
use std::path::Path;
use std::io::ErrorKind::{NotFound, AlreadyExists};
use std::fs::{File, OpenOptions, remove_file, create_dir, rename, metadata};

use serde_json::{to_writer_pretty, from_reader, Value as Json};
use serde::Serialize;


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
    let file = File::open(path)?;
    // TODO(tailhook) better error wrapping
    from_reader(BufReader::new(file)).map_err(
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

pub fn write_file<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
    try!(ensure_dir(&path.parent().expect("valid dir")));
    let tmppath = path.with_extension("tmp");
    to_writer_pretty(BufWriter::new(File::create(&tmppath)?), data)?;
    try!(rename(&tmppath, path));
    Ok(())
}
