use std::path::{PathBuf, Path};
use std::time::Duration;
use std::thread::sleep;
use std::process::exit;


use time::get_time;
use inotify::INotify;
use inotify::ffi::{IN_MODIFY, IN_ATTRIB, IN_CLOSE_WRITE, IN_MOVED_FROM};
use inotify::ffi::{IN_MOVED_TO, IN_CREATE, IN_DELETE, IN_DELETE_SELF};
use inotify::ffi::{IN_MOVE_SELF};
use scan_dir::ScanDir;

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

fn watch_dir(notify: &mut INotify, path: &Path) {
    ScanDir::dirs().walk(path, |iter| {
        for (entry, _) in iter {
            notify.add_watch(&entry.path(),
                IN_MODIFY | IN_ATTRIB | IN_CLOSE_WRITE | IN_MOVED_FROM |
                IN_MOVED_TO | IN_CREATE | IN_DELETE | IN_DELETE_SELF |
                IN_MOVE_SELF)
            .map_err(|e| {
                warn!("Error adding directory {:?} to inotify: {}.",
                      entry.path(), e);
            }).ok();
        }
    }).map_err(|e| {
        warn!("Error when scanning config directory: {:?}", e);
    }).ok();
}

pub fn main(state: SharedState, mut settings: Settings, mut alarm: Alarm) -> !
{
    let mut inotify = INotify::init().expect("create inotify");
    let _guard = ExitOnReturn(92);
    let mut scheduler = {
        let _alarm = alarm.after(Duration::from_secs(10));
        watch_dir(&mut inotify, &settings.config_dir);
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
            let mut events = inotify.available_events()
                .expect("read inotify")
                .len();
            if events > 0 {
                debug!("Inotify events, waiting to become stable");
                {
                    let _alarm = alarm.after(Duration::from_secs(10));
                    while events > 0 {
                        // Since we rescan every file anyway, it's negligible
                        // to just rescan the whole directory tree for inotify
                        // too
                        watch_dir(&mut inotify, &settings.config_dir);
                        // Wait a little bit for filesystem to become stable.
                        // We intentinally add new directories first, so that
                        // we can track unstable changes in new directories
                        // too.
                        // 200 ms should be enough for file copy/backup tools,
                        //     but not for human interaction, which is fine.
                        sleep(Duration::from_millis(200));
                        events = inotify.available_events()
                                 .expect("read inotify")
                                 .len();
                    }
                }

                debug!("Directories stable. Reading configs");
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

            let (result, dbg) = scheduler.execute(
                &*state.config(),
                &peers.1,
                &cookie.parent_schedules,
                &cookie.actions);

            let json = match result {
                Ok(json) => json,
                Err(e) => {
                    error!("Scheduling failed: {}", e);
                    state.set_schedule_debug_info(dbg);
                    sleep(Duration::from_secs(1));
                    continue;
                }
            };

            let hash = hash(json.to_string());
            state.set_schedule_by_leader(cookie, Schedule {
                timestamp: timestamp.to_msec(),
                hash: hash,
                data: json,
                origin: scheduler.id.clone(),
            }, dbg);
            break;
        }
    }
}
