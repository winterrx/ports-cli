use crate::model::ListenerEntry;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::process::{Command, Stdio};

pub fn listening_tcp_ports() -> Result<Vec<ListenerEntry>> {
    lsof_listeners().or_else(|_| ss_listeners())
}

fn lsof_listeners() -> Result<Vec<ListenerEntry>> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pn"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to run lsof")?;

    if !output.status.success() {
        bail!("lsof exited unsuccessfully")
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut listeners = BTreeMap::new();
    let mut current_pid = None;

    for line in stdout.lines() {
        if let Some(pid) = line
            .strip_prefix('p')
            .and_then(|value| value.parse::<u32>().ok())
        {
            current_pid = Some(pid);
            continue;
        }
        if let Some(name) = line.strip_prefix('n')
            && let Some(pid) = current_pid
            && let Some(port) = parse_port(name)
        {
            listeners.entry(port).or_insert(ListenerEntry { port, pid });
        }
    }

    Ok(listeners.into_values().collect())
}

fn ss_listeners() -> Result<Vec<ListenerEntry>> {
    let output = Command::new("ss")
        .args(["-H", "-ltnp"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to run ss")?;

    if !output.status.success() {
        bail!("ss exited unsuccessfully")
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut listeners = BTreeMap::new();

    for line in stdout.lines() {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        if columns.len() < 5 {
            continue;
        }

        let Some(port) = parse_port(columns[3]) else {
            continue;
        };
        let Some(pid) = extract_ss_pid(line) else {
            continue;
        };
        listeners.entry(port).or_insert(ListenerEntry { port, pid });
    }

    Ok(listeners.into_values().collect())
}

fn parse_port(value: &str) -> Option<u16> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = trimmed.rsplit(':').next()?.trim_matches(']').trim();
    candidate.parse::<u16>().ok().filter(|port| *port != 0)
}

fn extract_ss_pid(line: &str) -> Option<u32> {
    let marker = "pid=";
    let start = line.find(marker)? + marker.len();
    let digits = line[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse::<u32>().ok()
}
