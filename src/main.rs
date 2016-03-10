extern crate argparse;
extern crate handlebars;
extern crate env_logger;
extern crate quire;
extern crate rustc_serialize;
extern crate tempfile;
extern crate time;
extern crate rand;
extern crate libc;
extern crate lua;
extern crate nix;
extern crate scan_dir;
extern crate yaml_rust;
extern crate cbor;
#[macro_use] extern crate rotor;
extern crate rotor_http;
extern crate rotor_tools;
extern crate rotor_cantal;
#[macro_use] extern crate log;
#[macro_use] extern crate matches;
#[macro_use] extern crate quick_error;

use rand::Rng;
use time::now_utc;
use std::net::ToSocketAddrs;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::process::exit;

mod path_util;
mod fs_util;
mod config;
mod render;
mod apply;
mod scheduler;
mod elect;
mod frontend;
mod net;
mod info;

use argparse::{ArgumentParser, Parse, ParseOption, StoreOption, StoreTrue};

pub struct Options {
    config_dir: PathBuf,
    log_dir: PathBuf,
    dry_run: bool,
    print_configs: bool,
    hostname: Option<String>,
    listen_host: String,
    listen_port: u16,
    machine_id: Option<elect::Id>,
}

fn init_logging(id: &elect::Id) {
    use std::env;
    use log::{LogLevelFilter, LogRecord};
    use env_logger::LogBuilder;

    let id = format!("{}", id);
    let format = move |record: &LogRecord| {
        format!("{} {} {}: {}", now_utc().rfc3339(),
            id, record.level(), record.args())
    };
    let mut builder = LogBuilder::new();
    builder.format(format).filter(None, LogLevelFilter::Warn);

    if let Ok(val) = env::var("RUST_LOG") {
       builder.parse(&val);
    }
    builder.init().unwrap();
}

fn main() {
    let mut options = Options {
        config_dir: PathBuf::from("/etc/verwalter"),
        log_dir: PathBuf::from("/var/log/verwalter"),
        dry_run: false,
        print_configs: false,
        hostname: None,
        listen_host: "127.0.0.1".to_string(),
        listen_port: 8379,
        machine_id: None,
    };
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut options.config_dir)
            .add_option(&["-D", "--config-dir"], Parse,
                "Directory of configuration files");
        ap.refer(&mut options.hostname)
            .add_option(&["--hostname"], ParseOption,
                "Hostname of current server");
        ap.refer(&mut options.machine_id)
            .add_option(&["--override-machine-id"], StoreOption,
                "Overrides machine id. Do not use in production, put the
                 file `/etc/machine-id` instead. This should only be used
                 for tests which run multiple nodes in single filesystem
                 image");
        ap.refer(&mut options.dry_run)
            .add_option(&["-n", "--dry-run"], StoreTrue, "
                Just try to render configs, and don't run anything real.
                Use with RUST_LOG=debug to find out every command that
                is about to run");
        ap.refer(&mut options.log_dir)
            .add_option(&["--log-dir"], Parse, "
                Directory for log files. The directory must be owned by
                verwalter, meaning that nobody should put any extra files
                there (because verwalter may traverse the directory)");
        ap.refer(&mut options.print_configs)
            .add_option(&["--print-configs"], StoreTrue, "
                Print all rendered configs to stdout. It's useful with dry-run
                because every temporary file will be removed at the end of
                run. Note configurations are printed to stdout not to the
                log.");
        ap.refer(&mut options.listen_host)
            .add_option(&["--host"], Parse, "
                Bind to host (ip), for web and cluster messaging");
        ap.refer(&mut options.listen_port)
            .add_option(&["--port"], Parse, "
                Bind to port, for web and cluster messaging");
        ap.parse_args_or_exit();
    }

    let id = options.machine_id.clone().unwrap_or_else(info::machine_id);
    init_logging(&id);

    let mut cfg_cache = config::Cache::new();
    let config = match config::read_configs(&options, &mut cfg_cache) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Fatal error while reading config: {}", e);
            exit(3);
        }
    };
    debug!("Configuration read with, roles: {}, meta items: {}, errors: {}",
        config.roles.len(),
        config.machine.as_ref().ok().and_then(|o| o.as_object())
            .map(|x| x.len()).unwrap_or(0),
        config.total_errors());
    let mut scheduler = match scheduler::read(&options.config_dir) {
        Ok(s) => s,
        Err(e) => {
            error!("Scheduler load failed: {}", e);
            exit(4);
        }
    };
    info!("Started with machine id {}", id);
    let addr = (&options.listen_host[..], options.listen_port)
        .to_socket_addrs().expect("Can't resolve hostname")
        .collect::<Vec<_>>()[0];
    net::main(&addr, id, Arc::new(RwLock::new(config)))
        .expect("Error running main loop");
}

fn execute_scheduler(scheduler: &mut scheduler::Scheduler,
                     config: &config::Config, options: &Options)
{
    debug!("Scheduler loaded");
    let scheduler_result = match scheduler.execute(&config) {
        Ok(j) => j,
        Err(e) => {
            error!("Initial scheduling failed: {}", e);
            exit(5);
        }
    };
    debug!("Got initial scheduling of {}", scheduler_result);
    let apply_task = match render::render_all(&config,
        &scheduler_result, &options.hostname.as_ref().unwrap(),
                            options.print_configs)
    {
        Ok(res) => res,
        Err(e) => {
            error!("Initial configuration render failed: {}", e);
            exit(5);
        }
    };
    if log_enabled!(log::LogLevel::Debug) {
        for (role, result) in &apply_task {
            match result {
                &Ok(ref v) => {
                    debug!("Role {:?} has {} apply tasks", role, v.len());
                }
                &Err(render::Error::Skip) => {
                    debug!("Role {:?} is skipped on the node", role);
                }
                &Err(ref e) => {
                    debug!("Role {:?} has error: {}", role, e);
                }
            }
        }
    }

    let id = rand::thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = apply::log::Index::new(&options.log_dir, options.dry_run);
    let mut dlog = index.deployment(id);
    dlog.object("config", &config);
    dlog.json("scheduler_result", &scheduler_result);
    let (rerrors, gerrs) = apply::apply_all(apply_task, dlog, options.dry_run);
    if log_enabled!(log::LogLevel::Debug) {
        for e in gerrs {
            error!("Error when applying config: {}", e);
        }
        for (role, errs) in rerrors {
            for e in errs {
                error!("Error when applying config for {:?}: {}", role, e);
            }
        }
    }
}
