use crate::cpu_cache::estimate_cpu_usage_and_store;
use crate::detect::{
    detect_framework_from_command, detect_framework_from_image, detect_framework_from_project,
    is_dev_process, is_dockerish, is_noise_process, project_root_from_snapshot_cached,
    summarize_command,
};
use crate::docker::docker_port_map;
use crate::listeners::listening_tcp_ports;
use crate::model::{KillTarget, KillVia, PortInfo, ProcessHealth, ProcessInfo, ProcessSnapshot};
use crate::system::{
    collect_process_snapshots, collect_process_snapshots_basic_cpu_time,
    collect_process_snapshots_for_pids, format_uptime, git_branch, pid_exists, process_tree,
};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn get_listening_ports(detailed: bool) -> Result<Vec<PortInfo>> {
    let listeners = listening_tcp_ports()?;
    let listener_pids = listeners
        .iter()
        .map(|listener| listener.pid)
        .collect::<Vec<_>>();
    let snapshots =
        collect_process_snapshots_for_pids(&listener_pids, false, if detailed { 8 } else { 1 });
    let needs_docker = listeners.iter().any(|listener| {
        snapshots
            .get(&listener.pid)
            .is_some_and(|process| is_dockerish(&process.name, &process.command))
    });
    let docker = needs_docker.then(docker_port_map).unwrap_or_default();
    let mut project_root_cache = HashMap::new();
    let mut framework_cache = HashMap::new();

    let mut ports = listeners
        .into_iter()
        .filter_map(|listener| {
            build_port_info(
                listener.port,
                listener.pid,
                detailed,
                &snapshots,
                &docker,
                &mut project_root_cache,
                &mut framework_cache,
            )
        })
        .collect::<Vec<_>>();
    ports.sort_by_key(|info| info.port);
    Ok(ports)
}

pub fn get_port_details(port: u16) -> Result<Option<PortInfo>> {
    let Some(listener) = listening_tcp_ports()?
        .into_iter()
        .find(|listener| listener.port == port)
    else {
        return Ok(None);
    };

    let snapshots = collect_process_snapshots_for_pids(&[listener.pid], false, 8);
    let docker = snapshots
        .get(&listener.pid)
        .is_some_and(|process| is_dockerish(&process.name, &process.command))
        .then(docker_port_map)
        .unwrap_or_default();
    let mut project_root_cache = HashMap::new();
    let mut framework_cache = HashMap::new();

    Ok(build_port_info(
        listener.port,
        listener.pid,
        true,
        &snapshots,
        &docker,
        &mut project_root_cache,
        &mut framework_cache,
    ))
}

pub fn get_all_processes() -> Vec<ProcessInfo> {
    let snapshots = collect_process_snapshots(true);
    let mut project_root_cache = HashMap::new();
    let mut framework_cache = HashMap::new();
    let mut processes = snapshots
        .values()
        .filter(|process| process.pid > 4 && process.pid != std::process::id())
        .map(|process| build_process_info(process, &mut project_root_cache, &mut framework_cache))
        .collect::<Vec<_>>();
    processes.sort_by(|left, right| right.cpu_usage.total_cmp(&left.cpu_usage));
    processes
}

pub fn get_dev_processes() -> Vec<ProcessInfo> {
    let snapshots = collect_process_snapshots_basic_cpu_time();
    let estimated_cpu_usage = estimate_cpu_usage_and_store(&snapshots);
    let dev_pids = snapshots
        .values()
        .filter(|snapshot| snapshot.pid > 4 && snapshot.pid != std::process::id())
        .filter(|snapshot| is_dev_process(&snapshot.name, &snapshot.command))
        .map(|snapshot| snapshot.pid)
        .collect::<Vec<_>>();
    let mut visible_pids = Vec::new();
    let mut hidden_helper_pids = Vec::new();

    for pid in dev_pids {
        let Some(snapshot) = snapshots.get(&pid) else {
            continue;
        };
        if is_noise_process(&snapshot.name, &snapshot.command) {
            hidden_helper_pids.push(pid);
        } else {
            visible_pids.push(pid);
        }
    }

    let path_snapshots = collect_process_snapshots_for_pids(&visible_pids, false, 0);
    let mut project_root_cache = HashMap::new();
    let mut framework_cache = HashMap::new();
    let mut docker_rows = Vec::new();
    let mut processes = Vec::new();

    for pid in visible_pids {
        let Some(snapshot) = snapshots.get(&pid) else {
            continue;
        };
        let enriched_snapshot = path_snapshots.get(&pid).unwrap_or(snapshot);
        let mut merged_snapshot = enriched_snapshot.clone();
        merged_snapshot.cpu_usage = estimated_cpu_usage.get(&pid).copied().unwrap_or(0.0);
        merged_snapshot.memory_bytes = snapshot.memory_bytes;
        merged_snapshot.started_at = snapshot.started_at;
        merged_snapshot.command = snapshot.command.clone();
        merged_snapshot.accumulated_cpu_time_ms = snapshot.accumulated_cpu_time_ms;
        merged_snapshot.start_time_seconds = snapshot.start_time_seconds;

        let process = build_process_info(
            &merged_snapshot,
            &mut project_root_cache,
            &mut framework_cache,
        );
        if is_dockerish(&process.process_name, &process.command) {
            docker_rows.push(process);
        } else {
            processes.push(process);
        }
    }

    if !hidden_helper_pids.is_empty() {
        let mut helper_count = 0usize;
        let mut total_cpu = 0.0f32;
        let mut total_memory = 0u64;
        let mut uptime = None;
        let helper_pid_anchor = hidden_helper_pids[0];

        for pid in hidden_helper_pids {
            let Some(snapshot) = snapshots.get(&pid) else {
                continue;
            };
            helper_count += 1;
            total_cpu += estimated_cpu_usage.get(&pid).copied().unwrap_or(0.0);
            total_memory += snapshot.memory_bytes;
            uptime = uptime.or_else(|| format_uptime(snapshot.started_at));
        }

        processes.push(ProcessInfo {
            pid: helper_pid_anchor,
            process_name: "Helpers".to_string(),
            command: String::new(),
            description: format!("{} hidden helper processes", helper_count),
            cpu_usage: total_cpu,
            memory_bytes: Some(total_memory),
            project_name: None,
            framework: None,
            uptime,
        });
    }

    if !docker_rows.is_empty() {
        let total_cpu = docker_rows.iter().map(|process| process.cpu_usage).sum();
        let total_memory = docker_rows
            .iter()
            .filter_map(|process| process.memory_bytes)
            .sum::<u64>();
        processes.push(ProcessInfo {
            pid: docker_rows[0].pid,
            process_name: "Docker".to_string(),
            command: String::new(),
            description: format!("{} processes", docker_rows.len()),
            cpu_usage: total_cpu,
            memory_bytes: Some(total_memory),
            project_name: None,
            framework: Some("Docker".to_string()),
            uptime: docker_rows[0].uptime.clone(),
        });
    }

    processes.sort_by(|left, right| right.cpu_usage.total_cmp(&left.cpu_usage));
    processes
}

pub fn find_orphaned_processes() -> Result<Vec<PortInfo>> {
    Ok(get_listening_ports(false)?
        .into_iter()
        .filter(|port| matches!(port.status, ProcessHealth::Orphaned | ProcessHealth::Zombie))
        .collect())
}

pub fn resolve_kill_target(target: u32) -> Result<Option<KillTarget>> {
    if target == 0 {
        return Ok(None);
    }
    if target <= u16::MAX as u32
        && let Some(listener) = listening_tcp_ports()?
            .into_iter()
            .find(|listener| listener.port == target as u16)
    {
        return Ok(Some(KillTarget {
            pid: listener.pid,
            via: KillVia::Port,
            port: Some(listener.port),
        }));
    }
    if pid_exists(target) {
        return Ok(Some(KillTarget {
            pid: target,
            via: KillVia::Pid,
            port: None,
        }));
    }
    Ok(None)
}

pub fn filtered_ports(ports: Vec<PortInfo>) -> Vec<PortInfo> {
    ports
        .into_iter()
        .filter(|port| is_dev_process(&port.process_name, &port.command))
        .collect()
}

fn build_process_info(
    snapshot: &ProcessSnapshot,
    project_root_cache: &mut HashMap<PathBuf, Option<PathBuf>>,
    framework_cache: &mut HashMap<PathBuf, Option<String>>,
) -> ProcessInfo {
    let framework_from_process = detect_framework_from_command(&snapshot.command, &snapshot.name);
    let (_, project_name, framework) = resolve_project_context(
        snapshot,
        framework_from_process,
        project_root_cache,
        framework_cache,
    );

    ProcessInfo {
        pid: snapshot.pid,
        process_name: snapshot.name.clone(),
        command: snapshot.command.clone(),
        description: summarize_command(&snapshot.command, &snapshot.name),
        cpu_usage: snapshot.cpu_usage,
        memory_bytes: Some(snapshot.memory_bytes),
        project_name,
        framework,
        uptime: format_uptime(snapshot.started_at),
    }
}

fn build_port_info(
    port: u16,
    pid: u32,
    detailed: bool,
    snapshots: &HashMap<u32, ProcessSnapshot>,
    docker: &HashMap<u16, crate::model::DockerContainer>,
    project_root_cache: &mut HashMap<PathBuf, Option<PathBuf>>,
    framework_cache: &mut HashMap<PathBuf, Option<String>>,
) -> Option<PortInfo> {
    let snapshot = snapshots.get(&pid)?;
    let status = process_status(snapshot, snapshots);

    if let Some(container) = docker.get(&port) {
        return Some(PortInfo {
            port,
            pid,
            process_name: "docker".to_string(),
            command: snapshot.command.clone(),
            cwd: None,
            project_name: Some(container.name.clone()),
            framework: Some(detect_framework_from_image(&container.image)),
            uptime: format_uptime(snapshot.started_at),
            status,
            memory_bytes: Some(snapshot.memory_bytes),
            git_branch: None,
            started_at: snapshot.started_at,
            process_tree: if detailed {
                process_tree(pid, snapshots)
            } else {
                Vec::new()
            },
        });
    }

    let framework_from_process = detect_framework_from_command(&snapshot.command, &snapshot.name);
    let (project_root, project_name, framework) = resolve_project_context(
        snapshot,
        framework_from_process,
        project_root_cache,
        framework_cache,
    );

    let mut info = PortInfo {
        port,
        pid,
        process_name: snapshot.name.clone(),
        command: snapshot.command.clone(),
        cwd: project_root.clone(),
        project_name,
        framework,
        uptime: format_uptime(snapshot.started_at),
        status,
        memory_bytes: Some(snapshot.memory_bytes),
        git_branch: None,
        started_at: snapshot.started_at,
        process_tree: Vec::new(),
    };

    if detailed {
        if let Some(path) = info.cwd.as_ref() {
            info.git_branch = git_branch(path);
        }
        info.process_tree = process_tree(pid, snapshots);
    }

    Some(info)
}

fn process_status(
    snapshot: &ProcessSnapshot,
    snapshots: &HashMap<u32, ProcessSnapshot>,
) -> ProcessHealth {
    if snapshot.is_zombie {
        return ProcessHealth::Zombie;
    }

    if is_dev_process(&snapshot.name, &snapshot.command)
        && let Some(parent_pid) = snapshot.parent_pid
        && parent_pid > 4
        && !snapshots.contains_key(&parent_pid)
    {
        return ProcessHealth::Orphaned;
    }

    ProcessHealth::Healthy
}

fn resolve_project_context(
    snapshot: &ProcessSnapshot,
    process_framework: Option<String>,
    project_root_cache: &mut HashMap<PathBuf, Option<PathBuf>>,
    framework_cache: &mut HashMap<PathBuf, Option<String>>,
) -> (Option<PathBuf>, Option<String>, Option<String>) {
    let project_root = project_root_from_snapshot_cached(snapshot, project_root_cache);
    let project_name = project_root
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(ToString::to_string);
    let framework = process_framework.or_else(|| {
        let root = project_root.as_ref()?;
        if let Some(cached) = framework_cache.get(root) {
            return cached.clone();
        }
        let detected = detect_framework_from_project(root);
        framework_cache.insert(root.clone(), detected.clone());
        detected
    });

    (project_root, project_name, framework)
}
