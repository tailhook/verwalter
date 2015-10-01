use std::io;
use std::io::{Write, Seek};
use std::io::ErrorKind::NotFound;
use std::fs::{OpenOptions, File};
use std::fs::{create_dir, read_link, remove_file, rename, metadata};
use std::os::unix::fs::symlink;
use std::os::unix::ffi::OsStrExt;
use std::io::SeekFrom::Current;
use std::path::{Path, PathBuf};

use time::now_utc;
use rustc_serialize::json::{Json, as_json, as_pretty_json};

use fs_util::{raceless_symlink, ensure_dir};


/// Rotates role log file at this boundary
const MAX_ROLE_LOG: u64 = 10 << 20;  // 10 Mebibytes


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
    dry_run: bool,
}

pub struct Deployment<'a> {
    index_file: File,
    index: &'a mut Index,
    id: String,
    segment: String,
    log: File,
}

pub struct Role<'a: 'b, 'b> {
    deployment: &'b mut Deployment<'a>,
    role: &'b str,
    segment: String,
    log: File,
}

pub struct Action<'a: 'b, 'b: 'c, 'c> {
    parent: &'c mut  Role<'a, 'b>,
    action: &'c str,
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
    pub fn new(log_dir: PathBuf, dry_run: bool) -> Index {
        Index {
            log_dir: log_dir,
            dry_run: dry_run,
        }
    }
    pub fn deployment<'x>(&'x mut self, id: String)
        -> Result<Deployment<'x>, Error>
    {
        let segment = format!("{}", now_utc().strftime("%Y%m%d").unwrap());
        let idx_file;
        let log_file;

        if self.dry_run {
            idx_file = try!(File::create("/dev/stdout"));
            log_file = try!(File::create("/dev/stdout"));
        } else {
            let index_dir = self.log_dir.join(".index");
            try!(ensure_dir(&index_dir));
            let global_dir = self.log_dir.join(".global");
            try!(ensure_dir(&global_dir));

            idx_file = try!(open_segment(
                &index_dir, &format!("index.{}.json", segment)));
            log_file = try!(open_segment(
                &global_dir, &format!("log.{}.txt", segment)));
        }

        let mut depl = Deployment {
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

impl<'a> Deployment<'a> {
    fn global_index_entry(&mut self, marker: &Marker)
        -> Result<String, Error>
    {
        let pos = if self.index.dry_run { 0 } else {
            try!(self.log.seek(Current(0)))
        };
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
    fn role_index_entry(&mut self, ptr: Pointer, marker: &Marker)
        -> Result<String, Error>
    {
        let time = now_utc().rfc3339().to_string();
        {
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
    pub fn metadata(&mut self, name: &str, value: &Json)
        -> Result<(), io::Error>
    {
        write!(&mut self.log,
            "+++ metadata start: {name:?} +++\n\
             {value}\n\
             +++ metadata end: {name:?} +++\n",
             name=name, value=as_pretty_json(value))
    }
    pub fn role<'x>(&'x mut self, name: &'x str)
        -> Result<Role<'a, 'x>, Error>
    {
        let segment;
        let mut log_file;

        if self.index.dry_run {
            segment = "dry-run".to_string();
            log_file = try!(File::create("/dev/stdout"));
        } else {
            let role_dir = self.index.log_dir.join(name);
            try!(ensure_dir(&role_dir));
            let tup = try!(check_log(&role_dir));
            segment = tup.0;
            log_file = tup.1;
        }

        let mut role = Role {
            deployment: self,
            role: name,
            segment: segment,
            log: log_file,
        };
        try!(role.entry(Marker::RoleStart));
        Ok(role)
    }
}

impl<'a> Drop for Deployment<'a> {
    fn drop(&mut self) {
        self.global_entry(Marker::DeploymentFinish)
            .map_err(|e| error!("Can't write to log: {}", e)).ok();
    }
}

impl<'a, 'b> Role<'a, 'b> {
    fn entry(&mut self, marker: Marker)
        -> Result<(), Error>
    {
        let pos = if self.deployment.index.dry_run { 0 } else {
            try!(self.log.seek(Current(0)))
        };
        let ptr = Pointer::Role(self.role, &self.segment, pos);
        let date = try!(self.deployment.role_index_entry(ptr, &marker));
        let row = format!("{date} {id} ------------ {marker:?} ----------- \n",
            date=date, id=self.deployment.id, marker=marker);
        try!(self.log.write_all(row.as_bytes()));
        Ok(())
    }
}

impl<'a, 'b> Drop for Role<'a, 'b> {
    fn drop(&mut self) {
        self.entry(Marker::RoleFinish)
            .map_err(|e| error!("Can't write to log: {}", e)).ok();
    }
}

fn check_log(dir: &Path) -> Result<(String, File), Error> {
    let link = dir.join("latest");
    let seg = match read_link(&link) {
        Ok(ref x) if x.starts_with("log.") && x.ends_with(".txt") => {
            x.file_name().and_then(|fname| {
                fname.to_str().and_then(|fstr| {
                    Some(fstr[4..fname.as_bytes().len()-4]
                        .to_string())
                })
            })
        }
        Ok(x) => {
            error!("errorneous segment");
            None
        }
        Err(ref e) if e.kind() == NotFound => { None }
        Err(e) => return Err(From::from(e)),
    };
    if let Some(sname) = seg {
        let mut log_file = try!(open_segment(
            &dir, &format!("log.{}.txt", &sname)));
        if try!(log_file.seek(Current(0))) < MAX_ROLE_LOG {
            return Ok((sname, log_file));
        }
    }
    let segment = format!("{}", now_utc()
        .strftime("%Y%m%d%H%M%S").unwrap());
    let name = format!("log.{}.txt", segment);
    let log_file = try!(open_segment(&dir, &name));
    try!(raceless_symlink(&name, &link));
    return Ok((segment, log_file));
}

fn open_segment(dir: &Path, name: &String) -> Result<File, Error> {
    let filename = dir.join(name);
    let link = dir.join("latest");
    try!(raceless_symlink(name, &link));
    let file = try!(OpenOptions::new().write(true).create(true)
        .open(&filename));
    Ok(file)
}
