use std::collections::{HashMap, BTreeMap};
use std::path::{PathBuf, Path};
use std::process::exit;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, SystemTime, Instant};

use serde;
use serde_json::{Value as Json};
use inotify::{Inotify};
use scan_dir::ScanDir;
use lua::GcOption;
use libcantal::{Counter, Integer};
use frontend::serialize::{serialize_timestamp};

use config;
use hash::hash;
use id::Id;
use peer::Peer;
use scheduler::Schedule;
use scheduler::state::num_roles;
use shared::{SharedState};
use time_util::ToMsec;
use watchdog;


lazy_static! {
    pub static ref SCHEDULING_TIME: Integer = Integer::new();
    pub static ref SCHEDULER_SUCCEEDED: Counter = Counter::new();
    pub static ref SCHEDULER_FAILED: Counter = Counter::new();
}

#[derive(Debug)]
pub struct Parent(Arc<Schedule>);

#[derive(Serialize, Debug)]
pub struct SchedulerInput {
    #[serde(serialize_with="serialize_timestamp")]
    now: SystemTime,
    current_host: String,
    current_id: Id,
    parents: Vec<Parent>,
    actions: BTreeMap<u64, Arc<Json>>,
    runtime: Arc<Json>,
    peers: HashMap<Id, Arc<Peer>>,
    metrics: HashMap<(), ()>,  // TODO(tailhook)
}

impl serde::Serialize for Parent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        self.0.data.serialize(serializer)
    }
}

pub struct Settings {
    pub id: Id,
    pub hostname: String,
    pub config_dir: PathBuf,
}

fn watch_dir(notify: &mut Inotify, path: &Path) {
    use inotify::WatchMask as M;
    notify.add_watch(&path,
        M::MODIFY | M::ATTRIB | M::CLOSE_WRITE | M::MOVED_FROM |
        M::MOVED_TO | M::CREATE | M::DELETE | M::DELETE_SELF |
        M::MOVE_SELF)
    .map_err(|e| {
        warn!("Error adding directory {:?} to inotify: {}.",
              path, e);
    }).ok();
    ScanDir::dirs().walk(path, |iter| {
        for (entry, _) in iter {
            notify.add_watch(&entry.path(),
                M::MODIFY | M::ATTRIB | M::CLOSE_WRITE | M::MOVED_FROM |
                M::MOVED_TO | M::CREATE | M::DELETE | M::DELETE_SELF |
                M::MOVE_SELF)
            .map_err(|e| {
                warn!("Error adding directory {:?} to inotify: {}.",
                      entry.path(), e);
            }).ok();
        }
    }).map_err(|e| {
        warn!("Error when scanning config directory: {:?}", e);
    }).ok();
}

pub fn main(state: SharedState, settings: Settings) -> !
{
    let mut inotify = Inotify::init().expect("create inotify");
    let mut inotify_buf = vec![0u8; 8192];
    let _guard = watchdog::ExitOnReturn(92);
    let mut scheduler = {
        let _alarm = watchdog::Alarm::new(Duration::new(10, 0));
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
        let _alarm = watchdog::Alarm::new(Duration::new(2, 0));
        watch_dir(&mut inotify, &settings.config_dir.join("runtime"));
        config::read_runtime(&settings.config_dir.join("runtime"))
    };
    loop {
        sleep(Duration::new(5, 0));
        let mut cookie = if let Some(cookie) = state.leader_cookie() {
            cookie
        } else {
            continue;
        };

        while state.refresh_cookie(&mut cookie) {

            // TODO(tailhook) we reread everything on every iteration this
            // is waste of resources but for small configurations will be
            // negligible. Let's implement inotify later on
            let mut events = inotify.read_events(&mut inotify_buf)
                .expect("read inotify")
                .count();
            if events > 0 {
                debug!("Inotify events, waiting to become stable");
                {
                    let _alarm = watchdog::Alarm::new(Duration::new(10, 0));
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
                        events = inotify.read_events(&mut inotify_buf)
                                 .expect("read inotify")
                                 .count();
                    }
                }

                debug!("Directories stable. Reading configs");
                {
                    let _alarm = watchdog::Alarm::new(Duration::new(10, 0));
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
                    let _alarm = watchdog::Alarm::new(Duration::new(2, 0));
                    runtime = config::read_runtime(
                        &settings.config_dir.join("runtime"))
                }
            }

            let peers = state.peers();
            // TODO(tailhook) check if peers are outdated

            let timestamp = SystemTime::now();
            let instant = Instant::now();
            let _alarm = watchdog::Alarm::new(Duration::new(1, 0));

            let input = SchedulerInput {
                now: timestamp,
                current_host: scheduler.hostname.clone(),
                current_id: scheduler.id.clone(),
                parents: cookie.parent_schedules.iter()
                  .map(|x| Parent(x.clone())).collect(),
                actions: cookie.actions.clone(),
                runtime: runtime.data.clone(),
                // TODO(tailhook) show runtime errors
                //("runtime_err".to_string(), runtime.errors.to_json()),
                peers: peers.peers.iter()
                    .map(|(id, p)| (id.clone(), p.get()))
                    .collect(),
                metrics: HashMap::new(),
                /* TODO(tailhook)
                    state.metrics()
                    .map(|x| Json::Object(x.items.iter()
                        .map(|(host, data)| (host.to_string(),
                            convert_metrics(data)))
                        .collect()))
                    .unwrap_or(Json::Null)),
                */
            };

            let (result, dbg) = scheduler.execute(&input);
            SCHEDULING_TIME.set((Instant::now() - instant).to_msec() as i64);

            let json = match result {
                Ok(json) => {
                    state.clear_error("scheduler");
                    SCHEDULER_SUCCEEDED.incr(1);
                    json
                }
                Err(e) => {
                    error!("Scheduling failed: {}", e);
                    state.set_error("scheduler", format!("{}", e));
                    state.set_schedule_debug_info(input, dbg);
                    SCHEDULER_FAILED.incr(1);
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
            }, input, dbg);

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
