extern crate nix;
extern crate rustc_serialize;
extern crate time;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate log;

mod fs_util;

use std::io;
use std::fmt::Debug;
use std::mem::replace;
use std::io::{Write, Seek};
use std::io::ErrorKind::NotFound;
use std::fs::{OpenOptions, File, read_link};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{FromRawFd, AsRawFd};
use std::io::SeekFrom;
use std::fmt::Arguments;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use time::now_utc;
use rustc_serialize::json::{Json, as_json, as_pretty_json};

use fs_util::{raceless_symlink, ensure_dir};


/// Rotates role log file at this boundary
const MAX_ROLE_LOG: u64 = 10 << 20;  // 10 Mebibytes


quick_error! {
    #[derive(Debug)]
    pub enum Error {
        OpenGlobal(err: io::Error) {
            cause(err)
            display("can't open global log or index: {}", err)
            description("error open global log")
        }
        WriteGlobal(err: io::Error) {
            cause(err)
            display("can't write global log: {}", err)
            description("error writing global log")
        }
        WriteIndex(err: io::Error) {
            cause(err)
            display("can't write index: {}", err)
            description("error writing index")
        }
        WriteRole(err: io::Error, role: String) {
            cause(err)
            display("can't write role: {}", err)
            description("error write role log")
        }
        OpenRole(err: io::Error, role: String) {
            cause(err)
            display("can't write role log: {}", err)
            display("can't open file role log")
        }
        Dup(err: nix::Error) {
            cause(err)
            display("can't dup file descriptor for logging: {}", err)
            display("can't dup file descriptor for logging")
        }
    }
}

pub struct Index {
    log_dir: PathBuf,
    stdout: bool,
}

pub struct Deployment<'a> {
    index_file: File,
    index: &'a mut Index,
    id: String,
    global_segment: String,
    log: File,
    errors: Vec<Error>,
    done: bool,
}

pub struct Role<'a: 'b, 'b> {
    deployment: &'b mut Deployment<'a>,
    role: &'b str,
    segment: String,
    log: File,
    err: Option<io::Error>,
    full_role: bool,
}

pub struct Action<'a: 'b, 'b: 'c, 'c> {
    role: &'c mut  Role<'a, 'b>,
    action: &'c str,
}

type IndexEntry<'a> = (/*date*/&'a str, /*deployment_id*/&'a str,
                       Pointer<'a>, &'a Marker<'a>);

#[derive(RustcEncodable)]
enum Pointer<'a> {
    Global(&'a str, u64),
    Role(&'a str, &'a str, u64),
    External(&'a str, u64),
}

#[derive(RustcEncodable, Debug)]
enum Marker<'a> {
    DeploymentStart,
    RoleStart,
    ActionStart(&'a str),
    ActionFinish(&'a str),
    ExternalLog,
    RoleFinish,
    DeploymentFinish,
    DeploymentError,
}

impl Index {
    pub fn new(log_dir: &Path, stdout: bool) -> Index {
        Index {
            log_dir: log_dir.to_path_buf(),
            stdout: stdout,
        }
    }
    pub fn deployment<'x>(&'x mut self, id: &str, start: bool)
        -> Deployment<'x>
    {
        let idx;
        let log;
        let segment;
        let mut errors = Vec::new();


        if self.stdout {
            segment = "dry-run".to_string();
            idx = open_stdout();
            log = open_stdout();
        } else {
            let index_dir = self.log_dir.join(".index");
            let idx_res = ensure_dir(&index_dir);

            let global_dir = self.log_dir.join(".global");
            let glob_res = ensure_dir(&global_dir);

            let file = idx_res.and_then(|()| {
                if start {
                    open_segment(&index_dir, &format!("index.{}.json",
                        now_utc().strftime("%Y%m%d").unwrap()))
                } else {
                    OpenOptions::new().create(true).append(true)
                    .open(&index_dir.join("latest"))
                }
            });
            match file {
                Ok(file) => idx = file,
                Err(e) => {
                    errors.push(Error::OpenGlobal(e));
                    idx = open_null();
                }
            }
            match glob_res.and_then(|()| check_log(&global_dir, start)) {
                Ok((seg, lg)) => {
                    segment = seg;
                    log = lg;
                }
                Err(e) => {
                    errors.push(Error::OpenGlobal(e));
                    log = open_null();
                    segment = "null".to_string();
                }
            }
        }

        let mut depl = Deployment {
            index_file: idx,
            index: self,
            id: id.to_string(),
            global_segment: segment,
            log: log,
            errors: errors,
            done: !start,
        };
        if start {
            depl.global_entry(Marker::DeploymentStart);
        }
        return depl;
    }
}

impl<'a> Deployment<'a> {
    fn global_index_entry(&mut self, marker: &Marker) -> String
    {
        let time = now_utc().rfc3339().to_string();
        let pos = if self.index.stdout { 0 } else {
            match self.log.seek(SeekFrom::End(0)) {
                Ok(x) => x,
                Err(e) => {
                    self.errors.push(Error::WriteGlobal(e));
                    return time;
                }
            }
        };
        {
            let ptr = Pointer::Global(&self.global_segment, pos);
            let entry: IndexEntry = (&time, &self.id, ptr, marker);
            let buf = format!("{}\n", as_json(&entry));
            // We rely on POSIX semantics of atomic writes, but even if that
            // doesn't work, it's better to continue writing entry instead of
            // having error. For now we are only writer anyway, so atomic
            // writing is only useful to not to confuse readers and to be
            // crash-safe.
            if let Err(e) = self.index_file.write_all(buf.as_bytes()) {
                self.errors.push(Error::WriteGlobal(e))
            }
        }
        return time;
    }
    fn role_index_entry(&mut self, ptr: Pointer, marker: &Marker) -> String {
        let time = now_utc().rfc3339().to_string();
        {
            let entry: IndexEntry = (&time, &self.id, ptr, marker);
            let buf = format!("{}\n", as_json(&entry));
            // We rely on POSIX semantics of atomic writes, but even if that
            // doesn't work, it's better to continue writing entry instead of
            // having error. For now we are only writer anyway, so atomic
            // writing is only useful to not to confuse readers and to be
            // crash-safe.
            if let Err(e) = self.index_file.write_all(buf.as_bytes()) {
                self.errors.push(Error::WriteIndex(e));
            }
        }
        time
    }
    fn global_entry(&mut self, marker: Marker) {
        let date = self.global_index_entry(&marker);
        let row = format!("{date} {id} ------------ {marker:?} ----------- \n",
            date=date, id=self.id, marker=marker);
        if let Err(e) = self.log.write_all(row.as_bytes()) {
            self.errors.push(Error::WriteGlobal(e));
        }
    }
    pub fn string(&mut self, name: &str, value: &str) {
        if let Err(e) = write!(&mut self.log,
             "{name}: {value}\n", name=name, value=value)
        {
             self.errors.push(Error::WriteGlobal(e));
        }
    }
    pub fn object(&mut self, name: &str, value: &Debug) {
        if let Err(e) = write!(&mut self.log,
            "+++ debug start: {name:?} +++\n\
             {value:#?}\n\
             +++ debug end: {name:?} +++\n",
             name=name, value=value)
        {
             self.errors.push(Error::WriteGlobal(e));
        }
    }
    pub fn text(&mut self, name: &str, value: &str) {
        if let Err(e) = write!(&mut self.log,
            "+++ debug start: {name:?} +++\n\
             {value}\n\
             +++ debug end: {name:?} +++\n",
             name=name, value=value)
        {
             self.errors.push(Error::WriteGlobal(e));
        }
    }
    pub fn json(&mut self, name: &str, value: &Json) {
        if let Err(e) = write!(&mut self.log,
            "+++ metadata start: {name:?} +++\n\
             {value}\n\
             +++ metadata end: {name:?} +++\n",
             name=name, value=as_pretty_json(value))
        {
             self.errors.push(Error::WriteGlobal(e));
        }
    }
    pub fn role<'x>(&'x mut self, name: &'x str, start: bool)
        -> Result<Role<'a, 'x>, Error>
    {
        let segment;
        let log_file;

        if self.index.stdout {
            segment = "dry-run".to_string();
            log_file = open_stdout();;
        } else {
            let role_dir = self.index.log_dir.join(name);
            match ensure_dir(&role_dir)
                .and_then(|()| check_log(&role_dir, start))
                .map_err(|e| Error::OpenRole(e, name.to_string()))
            {
                Ok((seg, log)) => {
                    segment = seg;
                    log_file = log;
                }
                Err(e) => {
                    self.errors.push(e);
                    segment = "stdout".to_string();
                    log_file = open_null();
                }
            }
        }

        let mut role = Role {
            deployment: self,
            role: name,
            segment: segment,
            log: log_file,
            full_role: start,
            err: None,
        };
        if start {
            role.entry(Marker::RoleStart);
        }
        Ok(role)
    }
    pub fn done(mut self) -> Vec<Error> {
        self.done = true;
        self.global_entry(Marker::DeploymentFinish);
        return replace(&mut self.errors, Vec::new());
    }
    pub fn errors(&self) -> &Vec<Error> {
        &self.errors
    }
    pub fn log(&mut self, args: Arguments) {
        if let Err(e) = self.log.write_fmt(args) {
             self.errors.push(Error::WriteGlobal(e));
        }
    }
}

impl<'a> Drop for Deployment<'a> {
    fn drop(&mut self) {
        if !self.done {
            self.global_entry(Marker::DeploymentError);
        }
    }
}

impl<'a, 'b> Role<'a, 'b> {
    fn entry(&mut self, marker: Marker) {
        let pos = if self.deployment.index.stdout { 0 } else {
            match self.log.seek(SeekFrom::End(0)) {
                Ok(x) => x,
                Err(e) => {
                    add_err(&mut self.err, Some(e));
                    return;
                }
            }
        };
        let ptr = Pointer::Role(self.role, &self.segment, pos);
        let date = self.deployment.role_index_entry(ptr, &marker);
        let row = format!(
            "{date} {id} ------------ {role}: {marker:?} ----------- \n",
            date=date, id=self.deployment.id, role=self.role, marker=marker);
        add_err(&mut self.err, self.log.write_all(row.as_bytes()).err());
    }
    pub fn action<'x>(&'x mut self, name: &'x str) -> Action<'a, 'b, 'x> {
        self.entry(Marker::ActionStart(name));
        Action {
            role: self,
            action: name,
        }
    }
    pub fn template(&mut self, source: &Debug, dest: &Path, value: &str) {
        if let Err(e) = write!(&mut self.log,
            "+++ template start: {source:?} -> {dest:?} +++\n\
             {value}\n\
             +++ template end: {source:?} -> {dest:?} +++\n",
             source=source, dest=dest, value=value)
        {
             self.deployment.errors.push(Error::WriteGlobal(e));
        }
    }
    pub fn log(&mut self, args: Arguments) {
        add_err(&mut self.err, self.log.write_fmt(args).err());
    }
}

impl<'a, 'b> Drop for Role<'a, 'b> {
    fn drop(&mut self) {
        if self.full_role {
            self.entry(Marker::RoleFinish);
        }
        if let Some(e) = self.err.take() {
            self.deployment.errors
                .push(Error::WriteRole(e, self.role.to_string()));
        }
    }
}

impl<'a, 'b, 'c> Action<'a, 'b, 'c> {
    pub fn log(&mut self, args: Arguments) {
        add_err(&mut self.role.err, self.role.log.write_fmt(args).err());
    }
    pub fn external_log(&mut self, path: &Path, position: u64) {
        self.role.log(format_args!("File position {:?}:{}\n",
            path, position));
        match path.to_str() {
            Some(path) => {
                let ptr = Pointer::External(path, position);
                self.role.deployment
                    .role_index_entry(ptr, &Marker::ExternalLog);
            }
            None => {
                self.role.log(format_args!(
                    "Bad file name for peek log {:?}\n", path));
            }
        }
    }
    pub fn error(&mut self, err: &::std::error::Error) {
        self.log(format_args!("Action error: {}\n", err));
    }
    pub fn redirect_command(&mut self, cmd: &mut Command)
        -> Result<(), Error>
    {
        let file = try!(nix::unistd::dup(self.role.log.as_raw_fd())
            .map_err(|e| Error::Dup(e)));
        cmd.stdout(unsafe { Stdio::from_raw_fd(file) });
        let file = try!(nix::unistd::dup(self.role.log.as_raw_fd())
            .map_err(|e| Error::Dup(e)));
        cmd.stderr(unsafe { Stdio::from_raw_fd(file) });
        Ok(())
    }
}

impl<'a, 'b, 'c> Drop for Action<'a, 'b, 'c> {
    fn drop(&mut self) {
        self.role.entry(Marker::ActionFinish(self.action));
    }
}

fn check_log(dir: &Path, do_rotate: bool)
    -> Result<(String, File), io::Error>
{
    let link = dir.join("latest");
    let seg = match read_link(&link) {
        Ok(ref path) => {
            let seg = path.file_name()
                .and_then(|fname| fname.to_str())
                .and_then(|fname| {
                    if fname.starts_with("log.") && fname.ends_with(".txt") {
                        Some(fname[4..fname.as_bytes().len()-4]
                            .to_string())
                    } else {
                        None
                    }
                });
            if seg.is_none() {
                error!("errorneous segment {:?}", path);
            }
            seg
        }
        Err(ref e) if e.kind() == NotFound => { None }
        Err(e) => return Err(e),
    };
    if let Some(sname) = seg {
        let mut log_file = try!(open_segment(
            &dir, &format!("log.{}.txt", &sname)));
        if !do_rotate || try!(log_file.seek(SeekFrom::End(0))) < MAX_ROLE_LOG {
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

fn open_segment(dir: &Path, name: &String) -> Result<File, io::Error> {
    let filename = dir.join(name);
    let link = dir.join("latest");
    try!(raceless_symlink(name, &link));
    let file = try!(OpenOptions::new().write(true).create(true)
        .append(true).open(&filename));
    Ok(file)
}

fn open_stdout() -> File {
    OpenOptions::new().truncate(false).append(true)
        .open("/dev/stdout")
        .expect("Can't open /dev/stdout ?")
}

fn open_null() -> File {
    OpenOptions::new().truncate(false).append(true)
        .open("/dev/null")
        .expect("Can't open /dev/null ?")
}

fn add_err(old: &mut Option<io::Error>, new: Option<io::Error>) {
    if let Some(e) = new {
        error!("Error writing deployment log: {}", e);
        if old.is_none() {
            *old = Some(e);
        }
    }
}
