use crate::model::{PortInfo, ProcessHealth, ProcessInfo, ProcessTreeNode};
use chrono::{DateTime, Local};
use comfy_table::{
    Cell, Color, ContentArrangement, Row, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL,
};
use owo_colors::OwoColorize;

pub fn display_port_table(ports: &[PortInfo], filtered: bool) {
    render_header();
    if ports.is_empty() {
        println!("  No active listening ports found.\n");
        return;
    }

    let mut table = base_table(vec![
        "PORT",
        "PROCESS",
        "PID",
        "PROJECT",
        "FRAMEWORK",
        "UPTIME",
        "STATUS",
    ]);
    for port in ports {
        table.add_row(Row::from(vec![
            Cell::new(format!(":{}", port.port)).fg(Color::White),
            Cell::new(truncate(&port.process_name, 18)),
            Cell::new(port.pid).fg(Color::DarkGrey),
            Cell::new(port.project_name.as_deref().unwrap_or("-")).fg(Color::Blue),
            framework_cell(port.framework.as_deref()),
            Cell::new(port.uptime.as_deref().unwrap_or("-")).fg(Color::Yellow),
            status_cell(&port.status),
        ]));
    }

    println!("{table}");
    println!();
    let suffix = if filtered {
        "  ·  --all to show everything"
    } else {
        ""
    };
    println!(
        "  {} port{} active  ·  Run ports <number> for details{}",
        ports.len(),
        if ports.len() == 1 { "" } else { "s" },
        suffix
    );
    println!();
}

pub fn display_process_table(processes: &[ProcessInfo], filtered: bool) {
    render_header();
    if processes.is_empty() {
        println!("  No dev processes found.\n");
        return;
    }

    let mut table = base_table(vec![
        "PID",
        "PROCESS",
        "CPU%",
        "MEM",
        "PROJECT",
        "FRAMEWORK",
        "UPTIME",
        "WHAT",
    ]);
    for process in processes {
        table.add_row(Row::from(vec![
            Cell::new(process.pid).fg(Color::DarkGrey),
            Cell::new(truncate(&process.process_name, 16)).fg(Color::White),
            cpu_cell(process.cpu_usage),
            Cell::new(format_bytes(process.memory_bytes)).fg(Color::Green),
            Cell::new(process.project_name.as_deref().unwrap_or("-")).fg(Color::Blue),
            framework_cell(process.framework.as_deref()),
            Cell::new(process.uptime.as_deref().unwrap_or("-")).fg(Color::Yellow),
            Cell::new(truncate(&process.description, 32)).fg(Color::DarkGrey),
        ]));
    }

    println!("{table}");
    println!();
    let suffix = if filtered {
        "  ·  --all to show everything"
    } else {
        ""
    };
    println!(
        "  {} process{}{}",
        processes.len(),
        if processes.len() == 1 { "" } else { "es" },
        suffix
    );
    println!();
}

pub fn display_port_detail(info: Option<&PortInfo>) {
    render_header();
    let Some(info) = info else {
        println!("  No process found on that port.\n");
        return;
    };

    println!("  Port :{}", info.port.to_string().bold());
    println!("  {}", "─".repeat(22).bright_black());
    println!();
    detail_row("Process", &info.process_name);
    detail_row("PID", &info.pid.to_string());
    detail_row("Status", &status_string(&info.status));
    detail_row("Framework", info.framework.as_deref().unwrap_or("-"));
    detail_row("Memory", &format_bytes(info.memory_bytes));
    detail_row("Uptime", info.uptime.as_deref().unwrap_or("-"));
    if let Some(started_at) = info.started_at {
        detail_row("Started", &format_started_at(started_at));
    }

    println!();
    println!("  {}", "Location".cyan().bold());
    println!("  {}", "─".repeat(22).bright_black());
    detail_row(
        "Directory",
        &info
            .cwd
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    detail_row("Project", info.project_name.as_deref().unwrap_or("-"));
    detail_row("Git Branch", info.git_branch.as_deref().unwrap_or("-"));

    if !info.process_tree.is_empty() {
        println!();
        println!("  {}", "Process Tree".cyan().bold());
        println!("  {}", "─".repeat(22).bright_black());
        for (index, node) in info.process_tree.iter().enumerate() {
            println!("  {}", tree_line(index, node, info.pid));
        }
    }

    println!();
    println!(
        "  Kill: ports kill {} or ports kill -f {}",
        info.port, info.port
    );
    println!();
}

pub fn display_clean_results(orphaned: &[PortInfo], killed: &[u32], failed: &[u32]) {
    render_header();
    if orphaned.is_empty() {
        println!("  No orphaned or zombie processes found.\n");
        return;
    }

    println!(
        "  Found {} orphaned/zombie process{}:\n",
        orphaned.len(),
        if orphaned.len() == 1 { "" } else { "es" }
    );

    for port in orphaned {
        let icon = if killed.contains(&port.pid) {
            "✓".green().to_string()
        } else if failed.contains(&port.pid) {
            "✕".red().to_string()
        } else {
            "?".yellow().to_string()
        };
        println!(
            "  {icon} :{} - {} (PID {})",
            port.port, port.process_name, port.pid
        );
    }

    println!();
    if !killed.is_empty() {
        println!(
            "  Cleaned {} process{}.",
            killed.len(),
            if killed.len() == 1 { "" } else { "es" }
        );
    }
    if !failed.is_empty() {
        println!(
            "  Failed to clean {} process{}.",
            failed.len(),
            if failed.len() == 1 { "" } else { "es" }
        );
    }
    println!();
}

pub fn display_watch_header() {
    render_header();
    println!("  {}", "Watching for port changes...".cyan().bold());
    println!("  Press Ctrl+C to stop\n");
}

pub fn display_watch_new(port: &PortInfo) {
    let timestamp = Local::now().format("%H:%M:%S").to_string();
    let project = port
        .project_name
        .as_ref()
        .map(|name| format!(" [{name}]"))
        .unwrap_or_default();
    let framework = port
        .framework
        .as_ref()
        .map(|name| format!(" {name}"))
        .unwrap_or_default();
    println!(
        "  {} {} :{} <- {}{}{}",
        timestamp.bright_black(),
        "NEW".green().bold(),
        port.port,
        port.process_name,
        project,
        framework
    );
}

pub fn display_watch_removed(port: u16) {
    let timestamp = Local::now().format("%H:%M:%S").to_string();
    println!(
        "  {} {} :{}",
        timestamp.bright_black(),
        "CLOSED".red().bold(),
        port
    );
}

fn render_header() {
    println!();
    println!(
        " {}",
        "┌─────────────────────────────────────┐".cyan().bold()
    );
    println!(
        " {}",
        "│  Port Whisperer                     │".cyan().bold()
    );
    println!(
        " {}",
        "│  listening to your ports...         │".bright_black()
    );
    println!(
        " {}",
        "└─────────────────────────────────────┘".cyan().bold()
    );
    println!();
}

fn base_table(headers: Vec<&str>) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(
            headers
                .into_iter()
                .map(|value| Cell::new(value).fg(Color::Cyan))
                .collect::<Vec<_>>(),
        );
    table
}

fn framework_cell(framework: Option<&str>) -> Cell {
    let label = framework.unwrap_or("-");
    let color = match label {
        "Next.js" => Color::White,
        "Bun" => Color::Yellow,
        "Vite" => Color::Yellow,
        "React" => Color::Cyan,
        "Vue" => Color::Green,
        "Angular" => Color::Red,
        "Astro" => Color::Magenta,
        "Rust" => Color::DarkYellow,
        "Python" => Color::Yellow,
        "Docker" | "PostgreSQL" | "MySQL" => Color::Blue,
        "Redis" => Color::Red,
        _ => Color::White,
    };
    Cell::new(label).fg(color)
}

fn status_cell(status: &ProcessHealth) -> Cell {
    match status {
        ProcessHealth::Healthy => Cell::new("● healthy").fg(Color::Green),
        ProcessHealth::Orphaned => Cell::new("● orphaned").fg(Color::Yellow),
        ProcessHealth::Zombie => Cell::new("● zombie").fg(Color::Red),
    }
}

fn status_string(status: &ProcessHealth) -> String {
    match status {
        ProcessHealth::Healthy => "healthy".to_string(),
        ProcessHealth::Orphaned => "orphaned".to_string(),
        ProcessHealth::Zombie => "zombie".to_string(),
    }
}

fn cpu_cell(cpu: f32) -> Cell {
    let color = if cpu > 25.0 {
        Color::Red
    } else if cpu > 5.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    Cell::new(format!("{cpu:.1}")).fg(color)
}

fn detail_row(label: &str, value: &str) {
    println!("  {:<16} {}", label.bright_black(), value);
}

fn tree_line(index: usize, node: &ProcessTreeNode, root_pid: u32) -> String {
    let indent = "  ".repeat(index);
    let prefix = if index == 0 { "→" } else { "└─" };
    let name = if node.pid == root_pid {
        node.name.clone().bold().to_string()
    } else {
        node.name.clone()
    };
    format!("{indent}{prefix} {name} ({})", node.pid)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let truncated = value
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>();
        format!("{truncated}…")
    }
}

fn format_bytes(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return "-".to_string();
    };
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn format_started_at(value: DateTime<Local>) -> String {
    value.format("%Y-%m-%d %H:%M:%S").to_string()
}
