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
use lua::GcOption;

use config;
use time_util::ToMsec;
use hash::hash;
use watchdog::{Alarm, ExitOnReturn};
use shared::{Id, SharedState};
use scheduler::Schedule;
use scheduler::state::num_roles;


pub struct Settings {
    pub id: Id,
    pub hostname: String,
    pub config_dir: PathBuf,
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

pub fn main(state: SharedState, settings: Settings, mut alarm: Alarm) -> !
{
    let mut inotify = INotify::init().expect("create inotify");
    let _guard = ExitOnReturn(92);
    let mut scheduler = {
        let _alarm = alarm.after(Duration::from_secs(10));
        watch_dir(&mut inotify, &settings.config_dir.join("scheduler"));
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
    let mut runtime = {
        let _alarm = alarm.after(Duration::from_secs(2));
        watch_dir(&mut inotify, &settings.config_dir.join("runtime"));
        config::read_runtime(&settings.config_dir.join("runtime"))
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
                {
                    let _alarm = alarm.after(Duration::from_secs(10));
                    match super::read(settings.id.clone(),
                                      settings.hostname.clone(),
                                      &settings.config_dir)
                    {
                        Ok(s) => {
                            scheduler = s;
                            state.clear_error("scheduler_load");
                        }
                        Err(e) => {
                            state.set_error("scheduler_load", format!("{}", e));
                            error!("Scheduler load failed: {}. \
                                Using the old one.", e);
                        }
                    }
                }
                {
                    let _alarm = alarm.after(Duration::from_secs(2));
                    runtime = config::read_runtime(
                        &settings.config_dir.join("runtime"))
                }
            }

            let peers = state.peers().expect("peers are ready for scheduler");
            // TODO(tailhook) check if peers are outdated

            let timestamp = get_time();
            let _alarm = alarm.after(Duration::from_secs(1));

            let (result, dbg) = scheduler.execute(
                &runtime,
                &peers.1,
                &cookie.parent_schedules,
                &cookie.actions,
                state.metrics());

            let json = match result {
                Ok(json) => {
                    state.clear_error("scheduler");
                    json
                }
                Err(e) => {
                    error!("Scheduling failed: {}", e);
                    state.set_error("scheduler", format!("{}", e));
                    state.set_schedule_debug_info(dbg);
                    sleep(Duration::from_secs(1));
                    continue;
                }
            };

            let hash = hash(json.to_string());
            state.set_schedule_by_leader(cookie, Schedule {
                num_roles: num_roles(&json),
                timestamp: timestamp.to_msec(),
                hash: hash,
                data: json,
                origin: scheduler.id.clone(),
            }, dbg);

            // We execute GC after every scheduler run, we are going to
            // sleep for quite a long time now, so don't care performance
            debug!("Garbage before collection: {}Kb, stack top: {}",
                scheduler.lua.gc(GcOption::Count, 0), scheduler.lua.get_top());
            scheduler.lua.gc(GcOption::Collect, 0);
            info!("Garbage after collection: {}Kb",
                scheduler.lua.gc(GcOption::Count, 0));
            break;
        }
    }
}
