use std::io;
use std::io::{Write, Seek};
use std::io::ErrorKind::{NotFound, AlreadyExists};
use std::fs::{OpenOptions, File};
use std::fs::{create_dir, read_link, remove_file, rename, metadata};
use std::os::unix::fs::symlink;
use std::os::unix::ffi::OsStrExt;
use std::io::SeekFrom::Current;
use std::path::{Path, PathBuf};

use time::now_utc;
use rustc_serialize::json::{as_json, as_pretty_json};

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Io(err: io::Error) {
            from() cause(err)
            display("can't write index: {}", err)
            description("error write index entry")
        }
    }
}

pub struct Index {
    log_dir: PathBuf,
}

pub struct DeploymentLog<'a> {
    index_file: File,
    index: &'a mut Index,
    id: String,
    segment: String,
    log: File,
}

pub struct RoleLog<'a> {
    deployment: &'a DeploymentLog<'a>,
    role: String,
    segment: String,
    log: File,
}

pub struct Action<'a> {
    parent: &'a RoleLog<'a>,
    action: String,
}

type IndexEntry<'a> = (/*date*/&'a str, /*deployment_id*/&'a str,
                       Pointer<'a>, &'a Marker<'a>);

#[derive(RustcEncodable)]
enum Pointer<'a> {
    Global(&'a str, u64),
    Role(&'a str, &'a str, u64),
}

#[derive(RustcEncodable, Debug)]
enum Marker<'a> {
    DeploymentStart,
    RoleStart,
    ActionStart(&'a str),
    ActionFinish(&'a str),
    RoleFinish,
    DeploymentFinish,
}

impl Index {
    fn start_deployment<'x>(&'x mut self, id: String)
        -> Result<DeploymentLog<'x>, Error>
    {
        let index_dir = self.log_dir.join(".index");
        try!(ensure_dir(&index_dir));
        let global_dir = self.log_dir.join(".global");
        try!(ensure_dir(&global_dir));

        let segment = format!("{}", now_utc().strftime("%Y%m%d").unwrap());
        let idx_file = try!(open_segment(
            &index_dir, &format!("index.{}.json", segment)));
        let log_file = try!(open_segment(
            &global_dir, &format!("log.{}.txt", segment)));

        let mut depl = DeploymentLog {
            index_file: idx_file,
            index: self,
            id: id,
            segment: segment,
            log: log_file,
        };
        try!(depl.global_entry(Marker::DeploymentStart));
        Ok(depl)
    }
}

impl<'a> DeploymentLog<'a> {
    fn global_index_entry(&mut self, marker: &Marker)
        -> Result<String, Error>
    {
        let pos = try!(self.log.seek(Current(0)));
        let time = now_utc().rfc3339().to_string();
        {
            let ptr = Pointer::Global(&self.segment, pos);
            let entry: IndexEntry = (&time, &self.id, ptr, marker);
            let mut buf = format!("{}\n", as_json(&entry));
            // We rely on POSIX semantics of atomic writes, but even if that
            // doesn't work, it's better to continue writing entry instead of
            // having error. For now we are only writer anyway, so atomic
            // writing is only useful to not to confuse readers and to be
            // crash-safe.
            try!(self.index_file.write_all(buf.as_bytes()));
        }
        Ok(time)
    }
    fn global_entry(&mut self, marker: Marker)
        -> Result<(), Error>
    {
        let date = try!(self.global_index_entry(&marker));
        let row = format!("{date} {id} ------------ {marker:?} ----------- \n",
            date=date, id=self.id, marker=marker);
        try!(self.log.write_all(row.as_bytes()));
        Ok(())
    }
    fn start_apply(role: &str, deployment_id: &str) {

    }
}

fn raceless_symlink(value: &String, dest: &Path) -> Result<(), io::Error> {
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

fn ensure_dir(dir: &Path) -> Result<(), io::Error> {
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

fn open_segment(dir: &Path, name: &String) -> Result<File, Error> {
    let filename = dir.join(name);
    let link = dir.join("latest");
    try!(raceless_symlink(name, &link));
    let file = try!(OpenOptions::new().write(true).create(true)
        .open(&filename));
    Ok(file)
}
