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
extern crate regex;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate rotor;
extern crate rotor_http;
extern crate rotor_tools;
extern crate rotor_cantal;
#[macro_use] extern crate log;
#[macro_use] extern crate matches;
#[macro_use] extern crate quick_error;

use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::process::exit;

use time::now_utc;
use shared::{Id, SharedState};

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
mod shared;
mod time_util;

use argparse::{ArgumentParser, Parse, ParseOption, StoreOption, StoreTrue};
use argparse::{Print};

pub struct Options {
    config_dir: PathBuf,
    log_dir: PathBuf,
    log_id: bool,
    dry_run: bool,
    print_configs: bool,
    hostname: Option<String>,
    listen_host: String,
    listen_port: u16,
    machine_id: Option<Id>,
}

fn init_logging(id: &Id, log_id: bool) {
    use std::env;
    use log::{LogLevelFilter, LogRecord};
    use env_logger::LogBuilder;

    let mut builder = LogBuilder::new();
    if log_id {
        let id = format!("{}", id);
        let format = move |record: &LogRecord| {
            format!("{} {} {}: {}", now_utc().rfc3339(),
                id, record.level(), record.args())
        };
        builder.format(format).filter(None, LogLevelFilter::Warn);
    }

    if let Ok(val) = env::var("RUST_LOG") {
       builder.parse(&val);
    }
    builder.init().unwrap();
}

fn main() {
    let mut options = Options {
        config_dir: PathBuf::from("/etc/verwalter"),
        log_dir: PathBuf::from("/var/log/verwalter"),
        log_id: false,
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
        ap.add_option(&["--version"],
            Print(env!("CARGO_PKG_VERSION").to_string()),
            "Show version and exit");
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
        ap.refer(&mut options.log_id)
            .add_option(&["--log-id"], StoreTrue, "
                Add own machine id to the log (Useful mostly for running
                containerized test in containers");
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
    init_logging(&id, options.log_id);

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
    let addr = (&options.listen_host[..], options.listen_port)
        .to_socket_addrs().expect("Can't resolve hostname")
        .collect::<Vec<_>>()[0];

    let state = SharedState::new(config);
    let hostname = options.hostname
                   .unwrap_or_else(|| info::hostname().expect("gethostname"));

    scheduler::spawn(state.clone(), scheduler::Settings {
        hostname: hostname.clone(),
        dry_run: options.dry_run,
        print_configs: options.print_configs,
        log_dir: options.log_dir,
        config_dir: options.config_dir.clone(),
    });

    info!("Started with machine id {}, listening {}", id, addr);
    net::main(&addr, id, hostname, state, options.config_dir.join("frontend"))
        .expect("Error running main loop");
}
