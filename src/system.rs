use crate::model::{ProcessSnapshot, ProcessTreeNode};
use chrono::{DateTime, Local, TimeZone};
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::thread;
use sysinfo::{
    MINIMUM_CPU_UPDATE_INTERVAL, Pid, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate,
    RefreshKind, System, UpdateKind,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CpuMode {
    None,
    TimeOnly,
    Usage,
}

pub fn collect_process_snapshots(include_cpu: bool) -> HashMap<u32, ProcessSnapshot> {
    collect_process_snapshots_inner(
        None,
        if include_cpu {
            CpuMode::Usage
        } else {
            CpuMode::None
        },
        true,
    )
}

pub fn collect_process_snapshots_basic_cpu_time() -> HashMap<u32, ProcessSnapshot> {
    collect_process_snapshots_inner(None, CpuMode::TimeOnly, false)
}

pub fn collect_process_snapshots_for_pids(
    pids: &[u32],
    include_cpu: bool,
    ancestor_depth: usize,
) -> HashMap<u32, ProcessSnapshot> {
    if pids.is_empty() {
        return HashMap::new();
    }

    let mut snapshots = collect_process_snapshots_inner(
        Some(pids),
        if include_cpu {
            CpuMode::Usage
        } else {
            CpuMode::None
        },
        true,
    );
    let mut seen = snapshots.keys().copied().collect::<HashSet<_>>();
    let mut frontier = snapshots
        .values()
        .filter_map(|snapshot| snapshot.parent_pid)
        .filter(|pid| *pid > 4)
        .collect::<Vec<_>>();

    for _ in 0..ancestor_depth {
        frontier.retain(|pid| *pid > 4 && seen.insert(*pid));
        if frontier.is_empty() {
            break;
        }

        let parent_snapshots =
            collect_process_snapshots_inner(Some(&frontier), CpuMode::None, false);
        frontier = parent_snapshots
            .values()
            .filter_map(|snapshot| snapshot.parent_pid)
            .filter(|pid| *pid > 4)
            .collect();
        snapshots.extend(parent_snapshots);
    }

    snapshots
}

pub fn process_tree(pid: u32, snapshots: &HashMap<u32, ProcessSnapshot>) -> Vec<ProcessTreeNode> {
    let mut tree = Vec::new();
    let mut current_pid = Some(pid);

    for _ in 0..8 {
        let Some(next_pid) = current_pid else {
            break;
        };
        let Some(process) = snapshots.get(&next_pid) else {
            break;
        };

        tree.push(ProcessTreeNode {
            pid: process.pid,
            name: process.name.clone(),
        });
        current_pid = process.parent_pid;
    }

    tree
}

pub fn pid_exists(pid: u32) -> bool {
    !collect_process_snapshots_for_pids(&[pid], false, 0).is_empty()
}

pub fn kill_process(pid: u32, force: bool) -> bool {
    #[cfg(windows)]
    {
        let mut command = Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if force {
            command.arg("/F");
        }

        return command.status().is_ok_and(|status| status.success());
    }

    #[cfg(unix)]
    {
        let signal = if force { "-9" } else { "-TERM" };
        return Command::new("kill")
            .args([signal, &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
    }

    #[cfg(not(any(windows, unix)))]
    {
        let _ = (pid, force);
        false
    }
}

pub fn git_branch(path: &std::path::Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", path.to_str()?, "rev-parse", "--abbrev-ref", "HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

pub fn format_uptime(started_at: Option<DateTime<Local>>) -> Option<String> {
    let started_at = started_at?;
    let duration = Local::now().signed_duration_since(started_at);
    let total_seconds = duration.num_seconds().max(0);
    let minutes = total_seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    Some(if days > 0 {
        format!("{days}d {}h", hours % 24)
    } else if hours > 0 {
        format!("{hours}h {}m", minutes % 60)
    } else if minutes > 0 {
        format!("{minutes}m {}s", total_seconds % 60)
    } else {
        format!("{total_seconds}s")
    })
}

fn process_start_time(seconds_since_epoch: u64) -> Option<DateTime<Local>> {
    if seconds_since_epoch == 0 {
        None
    } else {
        Local.timestamp_opt(seconds_since_epoch as i64, 0).single()
    }
}

fn collect_process_snapshots_inner(
    target_pids: Option<&[u32]>,
    cpu_mode: CpuMode,
    include_paths: bool,
) -> HashMap<u32, ProcessSnapshot> {
    if target_pids.is_some_and(|pids| pids.is_empty()) {
        return HashMap::new();
    }

    let mut system = System::new_with_specifics(RefreshKind::nothing());
    refresh_processes(&mut system, target_pids, cpu_mode, include_paths);

    if cpu_mode == CpuMode::Usage {
        thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        refresh_processes(&mut system, target_pids, cpu_mode, include_paths);
    }

    system
        .processes()
        .values()
        .map(|process| {
            let pid = process.pid().as_u32();
            let command = if process.cmd().is_empty() {
                process.name().to_string_lossy().into_owned()
            } else {
                process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(" ")
            };

            (
                pid,
                ProcessSnapshot {
                    pid,
                    parent_pid: process.parent().map(|parent| parent.as_u32()),
                    name: process.name().to_string_lossy().into_owned(),
                    command,
                    exe: process.exe().map(|path| path.to_path_buf()),
                    cwd: process.cwd().map(|path| path.to_path_buf()),
                    memory_bytes: process.memory(),
                    cpu_usage: if cpu_mode == CpuMode::Usage {
                        process.cpu_usage()
                    } else {
                        0.0
                    },
                    accumulated_cpu_time_ms: if cpu_mode == CpuMode::None {
                        0
                    } else {
                        process.accumulated_cpu_time()
                    },
                    start_time_seconds: process.start_time(),
                    started_at: process_start_time(process.start_time()),
                    is_zombie: matches!(process.status(), ProcessStatus::Zombie),
                },
            )
        })
        .collect()
}

fn refresh_processes(
    system: &mut System,
    target_pids: Option<&[u32]>,
    cpu_mode: CpuMode,
    include_paths: bool,
) {
    let mut refresh_kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cmd(UpdateKind::OnlyIfNotSet);
    if include_paths {
        refresh_kind = refresh_kind
            .with_cwd(UpdateKind::OnlyIfNotSet)
            .with_exe(UpdateKind::OnlyIfNotSet);
    }
    if cpu_mode != CpuMode::None {
        refresh_kind = refresh_kind.with_cpu();
    }

    let pid_storage =
        target_pids.map(|pids| pids.iter().copied().map(Pid::from_u32).collect::<Vec<_>>());
    let to_update = match pid_storage.as_deref() {
        Some(pids) => ProcessesToUpdate::Some(pids),
        None => ProcessesToUpdate::All,
    };

    system.refresh_processes_specifics(to_update, true, refresh_kind);
}
