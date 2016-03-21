use std::thread;
use std::path::PathBuf;
use std::time::Duration;
use std::process::exit;


use sha1::Sha1;
use time::get_time;

use watchdog::{Alarm, ExitOnReturn};
use shared::{SharedState, Schedule};


pub struct Settings {
    pub hostname: String,
    pub config_dir: PathBuf,
}

fn sha1<S: AsRef<[u8]>>(obj: S) -> String {
    let mut sha = Sha1::new();
    sha.update(obj.as_ref());
    return sha.hexdigest();
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
                debug!("Config did not change ({})", hash);
                continue;
            }
            info!("Got scheduling of {}: {}", hash, scheduler_result);

            scheduler.previous_schedule_hash = Some(hash.clone());
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
