extern crate argparse;
extern crate capturing_glob;
extern crate error_chain;
extern crate handlebars;
extern crate libc;
extern crate quire;
extern crate rand;
extern crate regex;
extern crate serde;
extern crate scan_dir;
extern crate serde_json;
extern crate serde_regex;
extern crate tempfile;
extern crate tera;
extern crate trimmer;
extern crate yaml_rust;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate quick_error;

extern crate indexed_log;
extern crate verwalter_config as config;

mod apply;
mod render;
mod renderfile;

use std::io::{BufReader};
use std::fs::File;
use std::path::{PathBuf};
use std::process::exit;

use argparse::{ArgumentParser, Parse, ParseOption, StoreTrue, FromCommandLine};
use serde_json::{Value, from_str as parse_json};

use indexed_log::Index;
use config::Sandbox;

struct ParseJson(Value);

impl FromCommandLine for ParseJson {
    fn from_argument(s: &str) -> Result<ParseJson, String> {
        parse_json(s).map_err(|x| x.to_string()).map(ParseJson)
    }
}


fn main() {
    let mut vars = ParseJson(Value::Null);
    let mut schedule = ParseJson(Value::Null);
    let mut schedule_file = None::<PathBuf>;
    let mut vars_file = None::<PathBuf>;
    let mut log_dir = PathBuf::from("/var/log/verwalter");
    let mut config_dir = PathBuf::from("/etc/verwalter");
    let mut check_dir = None::<PathBuf>;
    let mut dry_run = false;
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("
            Internal verwalter's utility to render it.
            With `-C` option can also be used to check whether templates
            can be rendered fine locally (where you don't have verwalter
            daemon).
        ");
        ap.refer(&mut vars).add_argument("vars", Parse, "
            Variables to pass to renderer (required unless ``--vars-file`` is
            specified).
            ");
        ap.refer(&mut schedule)
            .add_option(&["--schedule"], Parse, "
                Global variables to pass to global renderer.");
        ap.refer(&mut schedule_file)
            .add_option(&["--schedule-file"], ParseOption, "
                A file that contains global veriables to pass to renderer.
                This option conflicts with `--schedule`");
        ap.refer(&mut vars_file)
            .add_option(&["--vars-file"], ParseOption, "
                A file that contains veriables to pass to renderer.
                This option overrides `vars` argument");
        ap.refer(&mut check_dir)
            .add_option(&["-C", "--check", "--check-dir"], ParseOption, "
                Render things in specified log dir, and show output.
                This is used to check templates locally.
                Implies `--dry-run`. Doesn't touch `--log-dir`.
                Ignores so`template` key in var.");
        ap.refer(&mut dry_run).add_option(&["--dry-run"], StoreTrue, "
            Don't run commands just show the templates and command-lines.
            ");
        ap.refer(&mut log_dir).add_option(&["--log-dir"], Parse, "
            Log directory (default `/var/log/verwalter`)");
        ap.refer(&mut config_dir)
            .add_option(&["--config-dir"], Parse,
                "Directory of configuration files (default /etc/verwalter)");
        ap.parse_args_or_exit();
    }
    if check_dir.is_some() {
        dry_run = true;
    }
    let mut log = Index::new(&log_dir, dry_run);

    if let Some(filename) = vars_file {
        let res = File::open(&filename)
            .map(|f| BufReader::new(f)).map_err(|e| e.to_string())
            .and_then(|f| serde_json::from_reader(f)
                          .map_err(|e| e.to_string()));
        vars = match res {
            Ok(json) => ParseJson(json),
            Err(e) => {
                eprintln!("Error reading {:?}: {}", filename, e);
                exit(3);
            }
        }
    };
    let mut vars = match vars {
        ParseJson(Value::Object(v)) => v,
        ParseJson(v) => {
            eprintln!("`vars` argument or contents of `--vars-file` should be \
                an object, got {:.16} instead", format!("{:?}", v));
            exit(3);
        }
    };

    let (id, dir, sandbox) = if let Some(dir) = check_dir {
        ("dry-run-dep-id-dead-beef".into(), dir, Sandbox::empty())
    } else {
        let id = match vars.get("deployment_id").and_then(|x| x.as_str()) {
            Some(x) => x.to_string(),
            None => exit(3),
        };
        let template = match vars.get("template").and_then(|x| x.as_str()) {
            Some(x) => x.to_string(),
            None => exit(4),
        };
        match vars.get("verwalter_version").and_then(|x| x.as_str()) {
            Some(concat!("v", env!("CARGO_PKG_VERSION"))) => {},
            Some(_) => exit(5),
            None => exit(3),
        };
        let sandbox = match Sandbox::parse_all(&config_dir.join("sandbox")) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Error reading sandbox config: {}", e);
                exit(3);
            }
        };
        (id, config_dir.join("templates").join(template), sandbox)
    };

    let role = match vars.get("role").and_then(|x| x.as_str()) {
        Some(x) => x.to_string(),
        None => exit(3),
    };
    if let Some(filename) = schedule_file {
        let res = File::open(&filename)
            .map(|f| BufReader::new(f)).map_err(|e| e.to_string())
            .and_then(|f| serde_json::from_reader(f)
                          .map_err(|e| e.to_string()));
        let schedule: Value = match res {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Error reading {:?}: {}", filename, e);
                exit(3);
            }
        };
        vars.insert(String::from("full_schedule"), schedule);
    } else {
        vars.insert(String::from("full_schedule"), schedule.0);
    }

    let mut dlog = log.deployment(&id, false);
    {
        let mut rlog = match dlog.role(&role, false) {
            Ok(rlog) => rlog,
            Err(_) => exit(81),
        };
        match render::render_role(&dir, &Value::Object(vars), &mut rlog)
        {
            Err(e) => {
                rlog.log(format_args!(
                    "ERROR: Can't render templates: {}\n", e));
                // TODO(tailhook) should we still check dlog.errors()
                exit(10);
            }
            Ok(actions) => {
                match apply::apply_list(&role, actions, &mut rlog, dry_run,
                        &sandbox)
                {
                    Err(e) => {
                        rlog.log(format_args!(
                            "ERROR: Can't apply templates: {}\n", e));
                        // TODO(tailhook) should we still check dlog.errors()
                        exit(20);
                    }
                    Ok(()) => {}
                }
            }
        }
    }
    if dlog.errors().len() != 0 {
        exit(81);
    }
}
