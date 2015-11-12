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
extern crate scan_dir;
extern crate yaml_rust;
extern crate rotor;
extern crate rotor_http;
extern crate mio;
extern crate hyper;
#[macro_use] extern crate matches;
#[macro_use] extern crate log;
#[macro_use] extern crate quick_error;

use rand::Rng;
use std::process::exit;
use std::path::PathBuf;
use std::net::SocketAddr;

mod path_util;
mod routing_util;
mod fs_util;
mod config;
mod render;
mod apply;
mod scheduler;
mod elect;
mod net;

use argparse::{ArgumentParser, Parse, Store, StoreTrue};

pub struct Options {
    config_dir: PathBuf,
    log_dir: PathBuf,
    dry_run: bool,
    print_configs: bool,
    hostname: String,
    listen_web: SocketAddr,
}


fn main() {
    env_logger::init().unwrap();
    let mut options = Options {
        config_dir: PathBuf::from("/etc/verwalter"),
        log_dir: PathBuf::from("/var/log/verwalter"),
        dry_run: false,
        print_configs: false,
        hostname: "localhost".to_string(),
        listen_web: "127.0.0.1:8379".parse().unwrap(),
    };
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut options.config_dir)
            .add_option(&["-D", "--config-dir"], Parse,
                "Directory of configuration files");
        ap.refer(&mut options.hostname)
            .add_option(&["--hostname"], Parse,
                "Hostname of current server");
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
        ap.refer(&mut options.listen_web)
            .add_option(&["--listen-web"], Store,
                "Hostname and port of web frontend");
        ap.parse_args_or_exit();
    }
    match net::main(&options.listen_web) {
        Ok(()) => {}
        Err(e) => {
            error!("Error running main loop: {:?}", e);
            exit(1);
        }
    }
}

fn render_configs(options: Options) {
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
        &scheduler_result, options.hostname, options.print_configs)
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
    let mut index = apply::log::Index::new(options.log_dir, options.dry_run);
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
