use std::path::PathBuf;
use std::time::Duration;
use std::thread::sleep;
use std::process::exit;


use time::get_time;

use config;
use time_util::ToMsec;
use hash::hash;
use watchdog::{Alarm, ExitOnReturn};
use shared::{Id, SharedState};
use scheduler::Schedule;


pub struct Settings {
    pub id: Id,
    pub hostname: String,
    pub config_dir: PathBuf,
    pub config_cache: config::Cache,
}


pub fn main(state: SharedState, mut settings: Settings, mut alarm: Alarm) -> !
{
    let _guard = ExitOnReturn(92);
    let mut scheduler = {
        let _alarm = alarm.after(Duration::from_secs(10));
        match super::read(settings.id.clone(),
                          settings.hostname.clone(),
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
        let mut cookie = state.wait_schedule_update(Duration::from_secs(5));

        while state.refresh_cookie(&mut cookie) {

            // TODO(tailhook) we reread everything on every iteration this
            // is waste of resources but for small configurations will be
            // negligible. Let's implement inotify later on
            if true {
                let _alarm = alarm.after(Duration::from_secs(10));
                match super::read(settings.id.clone(),
                                  settings.hostname.clone(),
                                  &settings.config_dir)
                {
                    Ok(s) => {
                        scheduler = s;
                    }
                    Err(e) => {
                        error!("Scheduler load failed: {}. Using the old one.",
                               e);
                    }
                }
                match config::read_configs(
                    &settings.config_dir, &mut settings.config_cache)
                {
                    Ok(cfg) => {
                        state.set_config(cfg);
                    }
                    Err(e) => {
                        error!("Fatal error while reading config: {}. \
                            Using the old one", e);
                    }
                };
            }

            let peers = state.peers().expect("peers are ready for scheduler");
            // TODO(tailhook) check if peers are outdated

            let timestamp = get_time();
            let _alarm = alarm.after(Duration::from_secs(1));

            let scheduler_result = scheduler.execute(
                &*state.config(),
                &peers.1,
                &cookie.parent_schedules,
                &cookie.actions);

            let scheduler_result = match scheduler_result {
                Ok(j) => j,
                Err(e) => {
                    error!("Scheduling failed: {}", e);
                    sleep(Duration::from_secs(1));
                    continue;
                }
            };

            let hash = hash(scheduler_result.to_string());
            state.set_schedule_by_leader(cookie, Schedule {
                timestamp: timestamp.to_msec(),
                hash: hash,
                data: scheduler_result,
                origin: scheduler.id.clone(),
            });
            break;
        }
    }
}
