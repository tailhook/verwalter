use std::thread;
use std::path::PathBuf;
use std::time::Duration;
use std::process::exit;
use std::collections::HashMap;


use log;
use sha1::Sha1;
use rand::{thread_rng, Rng};
use rustc_serialize::json::Json;
use time::get_time;

use watchdog::{Alarm, ExitOnReturn};
use config::Config;
use shared::{Id, Peer, SharedState, Schedule};
use render;
use apply;


pub struct Settings {
    pub print_configs: bool,
    pub hostname: String,
    pub dry_run: bool,
    pub log_dir: PathBuf,
    pub config_dir: PathBuf,
}

fn sha1<S: AsRef<[u8]>>(obj: S) -> String {
    let mut sha = Sha1::new();
    sha.update(obj.as_ref());
    return sha.hexdigest();
}


fn apply_schedule(config: &Config, hash: &String, scheduler_result: &Json,
    peers: &HashMap<Id, Peer>, settings: &Settings)
{
    let apply_task = match render::render_all(config,
        &scheduler_result, &settings.hostname,
                            settings.print_configs)
    {
        Ok(res) => res,
        Err(e) => {
            error!("Configuration render failed: {}", e);
            return;
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
    dlog.string("schedule-hash", &hash);
    dlog.object("config", &config);
    dlog.object("peers", &peers);
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

pub fn main(state: SharedState, settings: Settings, mut alarm: Alarm) -> ! {
    let _guard = ExitOnReturn(92);
    let mut scheduler = {
        let _alarm = alarm.after(Duration::from_secs(10));
        match super::read(settings.hostname.clone(),
                                              &settings.config_dir)
        {
            Ok(s) => s,
            Err(e) => {
                error!("Scheduler load failed: {}", e);
                exit(4);
            }
        }
    };
    loop {
        thread::sleep(Duration::new(10, 0));
        if !state.election().is_leader {
            trace!("Not a leader. Sleeping...");
            continue;
        }
        // TODO(tailhook) check if peers are outdated
        // TODO(tailhook) check if we have leadership established
        if let Some(peers) = state.peers() {
            let cfg = state.config();
            let timestamp = get_time();
            let _alarm = alarm.after(Duration::from_secs(1));
            let scheduler_result = match scheduler.execute(&*cfg, &peers.1) {
                Ok(j) => j,
                Err(e) => {
                    error!("Scheduling failed: {}", e);
                    continue;
                }
            };

            let hash = sha1(scheduler_result.to_string());
            if scheduler.previous_schedule_hash.as_ref() == Some(&hash) {
                debug!("Config did not change ({}) skipping render", hash);
                continue;
            }
            info!("Got scheduling of {}: {}", hash, scheduler_result);

            // TODO(tailhook) what should we do
            //                if render or application failed?
            scheduler.previous_schedule_hash = Some(hash.clone());

            apply_schedule(&*cfg, &hash, &scheduler_result,
                           &peers.1, &settings);
            state.set_schedule(Schedule {
                timestamp: timestamp,
                hash: hash,
                data: scheduler_result,
            });
        } else {
            warn!("No peers data, don't try to rebuild config");
        }
    }
}
