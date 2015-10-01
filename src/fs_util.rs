use std::io;
use std::path::Path;
use std::io::ErrorKind::{NotFound, AlreadyExists};
use std::fs::{create_dir, read_link, remove_file, rename, metadata};
use std::os::unix::fs::symlink;
use std::os::unix::ffi::OsStrExt;


pub fn raceless_symlink(value: &String, dest: &Path) -> Result<(), io::Error> {
    let tmpdest = dest.with_extension("tmp");
    loop {
        match read_link(dest) {
            Ok(ref x) if x.as_os_str().as_bytes() == value.as_bytes() => break,
            Ok(_) => {
                match remove_file(&tmpdest) {
                    Ok(()) => {}
                    Err(ref e) if e.kind() == NotFound => {}
                    Err(e) => return Err(e),
                }
            },
            Err(ref e) if e.kind() == NotFound => {}
            Err(e) => return Err(From::from(e)),
        }
        match symlink(value, &tmpdest) {
            Ok(()) => {}
            Err(ref e) if e.kind() == AlreadyExists => continue,
            Err(e) => return Err(e),
        }
        match rename(&tmpdest, dest) {
            Ok(()) => break,
            Err(ref e) if e.kind() == NotFound => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

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
