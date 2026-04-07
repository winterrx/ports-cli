use crate::model::DockerContainer;
use std::collections::HashMap;
use std::process::{Command, Stdio};

pub fn docker_port_map() -> HashMap<u16, DockerContainer> {
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Ports}}\t{{.Names}}\t{{.Image}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let Ok(output) = output else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    let mut ports = HashMap::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let mut parts = line.splitn(3, '\t');
        let port_spec = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default().trim();
        let image = parts.next().unwrap_or_default().trim();
        if name.is_empty() {
            continue;
        }

        for port in extract_host_ports(port_spec) {
            ports.entry(port).or_insert_with(|| DockerContainer {
                name: name.to_string(),
                image: image.to_string(),
            });
        }
    }

    ports
}

fn extract_host_ports(port_spec: &str) -> Vec<u16> {
    port_spec
        .split(',')
        .filter_map(|segment| segment.trim().split_once("->").map(|(lhs, _)| lhs))
        .filter_map(|lhs| lhs.rsplit(':').next())
        .filter_map(|port| port.trim_matches(']').parse::<u16>().ok())
        .collect()
}
