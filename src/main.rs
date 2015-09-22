extern crate walker;
extern crate argparse;
extern crate rumblebars;
extern crate env_logger;
extern crate quire;
extern crate rustc_serialize;
extern crate tempfile;
extern crate hlua;
#[macro_use] extern crate quick_error;
#[macro_use] extern crate log;

use std::path::PathBuf;
use std::process::exit;

mod path_util;
mod config;
mod render;
mod apply;
mod scheduler;

use argparse::{ArgumentParser, Parse, StoreTrue};

pub struct Options {
    config_dir: PathBuf,
    dry_run: bool,
    print_configs: bool,
}


fn main() {
    env_logger::init().unwrap();
    let mut options = Options {
        config_dir: PathBuf::from("/etc/verwalter"),
        dry_run: false,
        print_configs: false,
    };
    {
        let mut ap = ArgumentParser::new();
        ap.refer(&mut options.config_dir)
            .add_option(&["-D", "--config-dir"], Parse,
                "Directory of configuration files");
        ap.refer(&mut options.dry_run)
            .add_option(&["-n", "--dry-run"], StoreTrue, "
                Just try to render configs, and don't run anything real.
                Use with RUST_LOG=debug to find out every command that
                is about to run");
        ap.refer(&mut options.print_configs)
            .add_option(&["--print-configs"], StoreTrue, "
                Print all rendered configs to stdout. It's useful with dry-run
                because every temporary file will be removed at the end of
                run. Note configurations are printed to stdout not to the
                log.");
        ap.parse_args_or_exit();
    }
    let configs = match config::read_configs(options) {
        Ok(configs) => configs,
        Err(e) => {
            error!("Initial configuration load failed: {}", e);
            exit(2);
        }
    };
    debug!("Configuration read with, templates: {}, renderers: {}",
        configs.templates.len(), configs.renderers.len());
    let mut scheduler = match scheduler::read(&configs.options.config_dir) {
        Ok(s) => s,
        Err(e) => {
            error!("Scheduler load failed: {}", e);
            exit(3);
        }
    };
    debug!("Scheduler loaded");
    let scheduler_result = match scheduler.execute() {
        Ok(j) => j,
        Err(e) => {
            error!("Initial scheduling failed: {}", e);
            exit(4);
        }
    };
    debug!("Got initial scheduling of {}", scheduler_result);
    let apply_task = match render::render_all(&configs.renderers, &configs,
        scheduler_result)
    {
        Ok(res) => res,
        Err(e) => {
            error!("Initial configuration render failed: {}", e);
            exit(5);
        }
    };
    debug!("Rendered config, got {} tasks to apply", apply_task.len());
}
