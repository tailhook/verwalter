use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::{File, hard_link, remove_file};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, exit};
use std::sync::Arc;
use std::time::Duration;

use async_slot as slot;
use capturing_glob::{glob_with, MatchOptions};
use deflate::write::GzEncoder;
use deflate::Compression;
use failure::{Error, err_msg};
use time::now_utc;
use serde_json::{Value as Json};
use indexed_log::Index;
use futures::Stream;

use scheduler::{SchedulerInput, Schedule};
use fs_util::{write_file, safe_write};
use shared::{SharedState};
use watchdog;

const HOURLY_SNAPSHOTS: usize = 36;
const DAILY_SNAPSHOTS: usize = 14;
const WEEKLY_SNAPSHOTS: usize = 12;


pub struct Settings {
    pub hostname: String,
    pub dry_run: bool,
    pub use_sudo: bool,
    pub log_dir: PathBuf,
    pub config_dir: PathBuf,
    pub schedule_dir: PathBuf,
}

pub struct ApplyData {
    pub id: String,
    pub schedule: Arc<Schedule>,
    pub roles: BTreeMap<String, Json>,
}

fn decode_render_error(s: ExitStatus) -> Cow<'static, str> {
    match s.code() {
        // Please, keep the docs in `doc/running/exit_codes` up to date
        Some(2) => "argparse error (should be a bug or version mismatch)",
        Some(3) => "argument validation error \
                    (should be a bug or version mismatch)",
        Some(4) => "no `template` key found => fix the scheduler",
        Some(5) => "version mismatch => restart the verwalter daemon",
        Some(10) => "error rendering templates",
        Some(20) => "error executing commands",
        Some(81) => "error when doing logging",
        Some(_) => {
            return format!("unknown code {}, please report a bug!", s).into();
        }
        None => {
            return format!("dead on signal: {}", s).into();
        }
    }.into()
}

fn apply_schedule(hash: &String, is_new: bool,
    apply_task: ApplyData, settings: &Settings,
    debug_info: Arc<Option<(SchedulerInput, String)>>, state: &SharedState)
{
    let mut index = Index::new(&settings.log_dir, settings.dry_run);
    let mut dlog = index.deployment(&apply_task.id, true);
    dlog.string("schedule-hash", &hash);
    if is_new {
        if let Some((_, ref log)) = *debug_info {
            dlog.text("scheduler-debug", log);
        }

        dlog.changes(&hash[..8]).map(|mut changes| {
            apply_task.schedule.data.as_object()
                .and_then(|x| x.get("changes"))
                .and_then(|y| y.as_array())
                .map(|lst| {
                    for line in lst {
                        line.as_str().map(|val| {
                            changes.add_line(val);
                        });
                    }
                });
        }).map_err(|e| error!("Can't create changes log: {}", e)).ok();
    }

    let string_schedule = format!("{}", apply_task.schedule.data);
    state.reset_unused_roles(apply_task.roles.keys());
    for (role_name, vars) in apply_task.roles {
        let mut rlog = match dlog.role(&role_name, true) {
            Ok(l) => l,
            Err(e) => {
                error!("Can't create role log: {}", e);
                return;
            }
        };
        let vars = format!("{}", vars);
        rlog.log(format_args!("Template variables: {}\n", vars));

        let mut cmd = if settings.use_sudo {
            let mut cmd = Command::new("sudo");
            cmd.arg("verwalter_render");
            cmd
        } else {
            Command::new("verwalter_render")
        };
        cmd.arg("--log-dir");
        cmd.arg(&settings.log_dir);
        cmd.arg("--config-dir");
        cmd.arg(&settings.config_dir);

        {
            let fname = "/tmp/verwalter/vars-for-render.json";
            match safe_write(fname.as_ref(), vars.as_bytes()) {
                Ok(()) => {}
                Err(e) => {
                    error!("Can't write schedule file {:?}: {}",
                        fname, e);
                    return;
                }
            }
            cmd.arg("--vars-file");
            cmd.arg(fname);
        }

        {
            let fname = "/tmp/verwalter/schedule-for-render.json";
            match safe_write(fname.as_ref(), string_schedule.as_bytes()) {
                Ok(()) => {}
                Err(e) => {
                    error!("Can't write schedule file {:?}: {}",
                        fname, e);
                    return;
                }
            }
            cmd.arg("--schedule-file");
            cmd.arg(fname);
        }

        if settings.dry_run {
            cmd.arg("--dry-run");
        }
        debug!("Running {:?}", cmd);
        match cmd.status() {
            Ok(x) if x.success() => {
                rlog.log(format_args!("Rendered successfully\n"));
                state.reset_role_failure(&role_name);
            }
            Ok(status) => {
                state.mark_role_failure(&role_name);
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    verwalter_render {}\n", status));
                rlog.log(format_args!(
                    "Decoded verwalter render failure: {}\n",
                    decode_render_error(status)));
            }
            Err(e) => {
                state.mark_role_failure(&role_name);
                rlog.log(format_args!(
                    "ERROR: Error rendering role. \
                    Can't run verwalter_render: {}\n", e));
            }
        }
    }
    for err in dlog.done() {
        error!("Logging error: {}", err);
    }
}

fn list_backups(pat: &str) -> Vec<(String, PathBuf)> {
    glob_with(pat, &MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: true,
    })
    .map(|iter| {
        iter.flat_map(|entry| match entry {
            Ok(e) => e.group(1)
                .and_then(|x| x.to_str())
                .map(|x| x.to_string())
                .map(|x| (x, e.into())),
            Err(e) => {
                error!("Error listing backups: {}", e);
                None
            }
        })
        .collect()
    }).unwrap_or_else(|e| {
        error!("Error listing backups: {}", e);
        Vec::new()
    })
}

fn maintain_backups(dir: &Path) -> Result<(), Error> {
    let dir_str = dir.to_str()
        .ok_or_else(|| err_msg("storage dir is not valid utf-8"))?;
    let mut hourly = list_backups(&format!("{}/hourly-(*).json.gz", dir_str));
    let mut daily = list_backups(&format!("{}/daily-(*).json.gz", dir_str));
    let mut weekly = list_backups(&format!("{}/weekly-(*).json.gz", dir_str));
    let current_hour = now_utc().strftime("%Y%m%dT%H")?.to_string();
    let current_day = now_utc().strftime("%Y%m%d")?.to_string();
    let current_week = now_utc().strftime("%Yw%W")?.to_string();
    let need_hour = hourly.last()
        .map(|&(ref x, _)| x != &current_hour).unwrap_or(true);
    let need_day = daily.last()
        .map(|&(ref x, _)| x != &current_day).unwrap_or(true);
    let need_week = weekly.last()
        .map(|&(ref x, _)| x != &current_week).unwrap_or(true);
    if !need_hour && !need_day && !need_week {
        return Ok(());
    }
    let infile = File::open(&dir.join("schedule.json"))?;
    let tmp_path = dir.join("backup.tmp");
    let outfile = GzEncoder::new(
        File::create(&tmp_path)?,
        Compression::Best);
    io::copy(&mut {infile}, &mut {outfile})?;
    if need_hour {
        let dest = dir.join(&format!("hourly-{}.json.gz", current_hour));
        hard_link(&tmp_path, &dest)
            .map_err(|e| error!("Error hardlinking snapshot: {}", e)).ok();
        hourly.push((current_hour, dest));
    }
    if need_day {
        let dest = dir.join(&format!("daily-{}.json.gz", current_day));
        hard_link(&tmp_path, &dest)
            .map_err(|e| error!("Error hardlinking snapshot: {}", e)).ok();
        daily.push((current_day, dest));
    }
    if need_week {
        let dest = dir.join(&format!("weekly-{}.json.gz", current_week));
        hard_link(&tmp_path, &dest)
            .map_err(|e| error!("Error hardlinking snapshot: {}", e)).ok();
        weekly.push((current_week, dest));
    }
    remove_file(&tmp_path)
        .map_err(|e| error!("Error removing {:?}: {}", tmp_path, e)).ok();
    let del_hourly = hourly.len().saturating_sub(HOURLY_SNAPSHOTS);
    for (_, fname) in hourly.drain(..del_hourly) {
        remove_file(&fname)
            .map_err(|e| error!("Error removing {:?}: {}", fname, e)).ok();
    }
    let del_daily = daily.len().saturating_sub(DAILY_SNAPSHOTS);
    for (_, fname) in daily.drain(..del_daily) {
        remove_file(&fname)
            .map_err(|e| error!("Error removing {:?}: {}", fname, e)).ok();
    }
    let del_weekly = weekly.len().saturating_sub(WEEKLY_SNAPSHOTS);
    for (_, fname) in weekly.drain(..del_weekly) {
        remove_file(&fname)
            .map_err(|e| error!("Error removing {:?}: {}", fname, e)).ok();
    }
    Ok(())
}

pub fn run(state: SharedState, settings: Settings,
    tasks: slot::Receiver<ApplyData>)
    -> !
{
    let _guard = watchdog::ExitOnReturn(93);
    let mut prev_schedule = String::new();
    for task in tasks.wait() {
        let task = task.unwrap_or_else(|_| exit(93));
        let schedule = task.schedule.clone();
        let _alarm = watchdog::Alarm::new(Duration::new(180, 0), "apply");
        write_file(&settings.schedule_dir.join("schedule.json"),
            &*schedule)
            .map_err(|e| error!("Writing schedule failed: {:?}", e)).ok();
        apply_schedule(&schedule.hash, prev_schedule != schedule.hash,
            task, &settings,
            state.scheduler_debug_info(), &state);
        maintain_backups(&settings.schedule_dir)
            .map_err(|e| error!("Writing backup failed: {:?}", e)).ok();
        prev_schedule = schedule.hash.clone();
    }
    unreachable!();
}
