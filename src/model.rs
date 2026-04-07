use chrono::{DateTime, Local};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessHealth {
    Healthy,
    Orphaned,
    Zombie,
}

#[derive(Clone, Debug)]
pub struct ProcessTreeNode {
    pub pid: u32,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    pub command: String,
    pub cwd: Option<PathBuf>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
    pub status: ProcessHealth,
    pub memory_bytes: Option<u64>,
    pub git_branch: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub process_tree: Vec<ProcessTreeNode>,
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub process_name: String,
    pub command: String,
    pub description: String,
    pub cpu_usage: f32,
    pub memory_bytes: Option<u64>,
    pub project_name: Option<String>,
    pub framework: Option<String>,
    pub uptime: Option<String>,
}

#[derive(Clone, Debug)]
pub enum KillVia {
    Port,
    Pid,
}

#[derive(Clone, Debug)]
pub struct KillTarget {
    pub pid: u32,
    pub via: KillVia,
    pub port: Option<u16>,
}

#[derive(Clone, Debug)]
pub(crate) struct ListenerEntry {
    pub port: u16,
    pub pid: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct DockerContainer {
    pub name: String,
    pub image: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessSnapshot {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub command: String,
    pub exe: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub memory_bytes: u64,
    pub cpu_usage: f32,
    pub accumulated_cpu_time_ms: u64,
    pub start_time_seconds: u64,
    pub started_at: Option<DateTime<Local>>,
    pub is_zombie: bool,
}
