use std::thread;
use std::path::PathBuf;
use std::time::Duration;
use std::process::exit;
use std::collections::HashMap;

use log;
use rand::{thread_rng, Rng};
use super::{Scheduler};
use config::Config;
use shared::{Id, Peer, SharedState};
use render;
use apply;


pub struct Settings {
    pub print_configs: bool,
    pub hostname: String,
    pub dry_run: bool,
    pub log_dir: PathBuf,
    pub config_dir: PathBuf,
}


fn execute_scheduler(scheduler: &mut Scheduler, config: &Config,
    hosts: &HashMap<Id, Peer>, settings: &Settings)
{
    debug!("Scheduler loaded");
    let scheduler_result = match scheduler.execute(config, hosts) {
        Ok(j) => j,
        Err(e) => {
            error!("Initial scheduling failed: {}", e);
            exit(5);
        }
    };
    debug!("Got initial scheduling of {}", scheduler_result);
    let apply_task = match render::render_all(config,
        &scheduler_result, &settings.hostname,
                            settings.print_configs)
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

    let id = thread_rng().gen_ascii_chars().take(24).collect();
    let mut index = apply::log::Index::new(
        &settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(id);
    dlog.object("config", &config);
    dlog.json("scheduler_result", &scheduler_result);
    let (rerrors, gerrs) = apply::apply_all(apply_task, dlog,
        settings.dry_run);
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

fn main(state: SharedState, settings: Settings) {
    let mut scheduler = match super::read(&settings.config_dir) {
        Ok(s) => s,
        Err(e) => {
            error!("Scheduler load failed: {}", e);
            exit(4);
        }
    };
    loop {
        thread::sleep(Duration::new(10, 0));
        {
            // TODO(tailhook) check if peers are outdated
            // TODO(tailhook) check if we have leadership established
            if let Some(peers) = state.peers() {
                execute_scheduler(&mut scheduler, &*state.config(),
                    &peers.1, &settings);
            } else {
                warn!("No peers data, don't try to rebuild config");
            }
        }
    }
}

pub fn spawn(state: SharedState, settings: Settings) {
    thread::spawn(|| {
        main(state, settings)
    });
}
