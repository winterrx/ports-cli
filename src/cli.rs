use crate::model::{KillVia, PortInfo};
use crate::output::{
    display_clean_results, display_port_detail, display_port_table, display_process_table,
    display_watch_header, display_watch_new, display_watch_removed,
};
use crate::scanner::{
    filtered_ports, find_orphaned_processes, get_all_processes, get_dev_processes,
    get_listening_ports, get_port_details, resolve_kill_target,
};
use crate::system::kill_process;
use anyhow::{Result, bail};
use dialoguer::Confirm;
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

pub fn run(program: &str, raw_args: Vec<String>) -> Result<()> {
    let args = ParsedArgs::parse(program, raw_args)?;
    match args.command {
        Command::ShowPorts => show_ports(args.show_all),
        Command::Detail(port) => show_detail(port),
        Command::Ps => show_processes(args.show_all),
        Command::Clean => clean_orphaned(),
        Command::Kill { force, targets } => kill_targets(force, &targets),
        Command::Watch => watch_ports(args.show_all),
        Command::Help => {
            print_help();
            Ok(())
        }
    }
}

enum Command {
    ShowPorts,
    Detail(u16),
    Ps,
    Clean,
    Kill { force: bool, targets: Vec<String> },
    Watch,
    Help,
}

struct ParsedArgs {
    show_all: bool,
    command: Command,
}

impl ParsedArgs {
    fn parse(program: &str, raw_args: Vec<String>) -> Result<Self> {
        let mut show_all = false;
        let filtered = raw_args
            .into_iter()
            .filter(|arg| {
                let is_all = arg == "--all" || arg == "-a";
                if is_all {
                    show_all = true;
                }
                !is_all
            })
            .collect::<Vec<_>>();

        if program == "whoisonport" {
            if filtered.is_empty() || matches!(filtered[0].as_str(), "--help" | "-h" | "help") {
                return Ok(Self {
                    show_all: false,
                    command: Command::Help,
                });
            }
            return Ok(Self {
                show_all: false,
                command: Command::Detail(parse_port(&filtered[0])?),
            });
        }

        let command = match filtered.first().map(String::as_str) {
            None => Command::ShowPorts,
            Some("help" | "--help" | "-h") => Command::Help,
            Some("ps") => Command::Ps,
            Some("clean") => Command::Clean,
            Some("watch") => Command::Watch,
            Some("kill") => {
                let force = filtered.iter().any(|arg| arg == "-f" || arg == "--force");
                let targets = filtered
                    .iter()
                    .skip(1)
                    .filter(|arg| *arg != "-f" && *arg != "--force")
                    .cloned()
                    .collect::<Vec<_>>();
                if targets.is_empty() {
                    bail!("Usage: ports kill [-f|--force] <port|pid> [port|pid...]");
                }
                Command::Kill { force, targets }
            }
            Some(value) => Command::Detail(parse_port(value)?),
        };

        Ok(Self { show_all, command })
    }
}

fn show_ports(show_all: bool) -> Result<()> {
    let ports = get_listening_ports(false)?;
    let ports = if show_all {
        ports
    } else {
        filtered_ports(ports)
    };
    display_port_table(&ports, !show_all);
    Ok(())
}

fn show_detail(port: u16) -> Result<()> {
    let info = get_port_details(port)?;
    display_port_detail(info.as_ref());

    let Some(info) = info else {
        return Ok(());
    };
    if !interactive() {
        return Ok(());
    }

    let should_kill = Confirm::new()
        .with_prompt(format!("Kill process on :{port}?"))
        .default(false)
        .interact()?;

    if should_kill {
        if kill_process(info.pid, false) {
            println!("\n  Killed PID {}\n", info.pid);
        } else {
            println!("\n  Failed to kill PID {}\n", info.pid);
        }
    }

    Ok(())
}

fn show_processes(show_all: bool) -> Result<()> {
    let processes = if show_all {
        get_all_processes()
    } else {
        get_dev_processes()
    };
    display_process_table(&processes, !show_all);
    Ok(())
}

fn clean_orphaned() -> Result<()> {
    let orphaned = find_orphaned_processes()?;
    if orphaned.is_empty() {
        display_clean_results(&orphaned, &[], &[]);
        return Ok(());
    }
    if !interactive() {
        display_clean_results(&orphaned, &[], &[]);
        return Ok(());
    }

    let should_kill = Confirm::new()
        .with_prompt(format!(
            "Kill all {} orphaned/zombie processes?",
            orphaned.len()
        ))
        .default(false)
        .interact()?;

    let mut killed = Vec::new();
    let mut failed = Vec::new();
    if should_kill {
        for port in &orphaned {
            if kill_process(port.pid, false) {
                killed.push(port.pid);
            } else {
                failed.push(port.pid);
            }
        }
    }

    display_clean_results(&orphaned, &killed, &failed);
    Ok(())
}

fn kill_targets(force: bool, targets: &[String]) -> Result<()> {
    let mut any_failed = false;
    println!();

    for target in targets {
        let Ok(parsed) = target.parse::<u32>() else {
            println!("  {target} is not a valid port or PID");
            any_failed = true;
            continue;
        };

        match resolve_kill_target(parsed)? {
            Some(target) => {
                let label = match target.via {
                    KillVia::Port => {
                        format!(":{} (PID {})", target.port.unwrap_or_default(), target.pid)
                    }
                    KillVia::Pid => format!("PID {}", target.pid),
                };
                println!("  Killing {label}");
                if kill_process(target.pid, force) {
                    println!(
                        "  Sent {} to {label}",
                        if force { "SIGKILL" } else { "SIGTERM" }
                    );
                } else {
                    println!("  Failed to kill {label}");
                    any_failed = true;
                }
            }
            None => {
                println!("  No listener or process found for {parsed}");
                any_failed = true;
            }
        }
    }

    println!();
    if any_failed {
        bail!("one or more kills failed");
    }
    Ok(())
}

fn watch_ports(show_all: bool) -> Result<()> {
    display_watch_header();
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
    })?;

    let mut previous = BTreeMap::<u16, PortInfo>::new();
    while running.load(Ordering::SeqCst) {
        let ports = get_listening_ports(false)?;
        let ports = if show_all {
            ports
        } else {
            filtered_ports(ports)
        };
        let current = ports
            .into_iter()
            .map(|port| (port.port, port))
            .collect::<BTreeMap<_, _>>();

        for (port, info) in &current {
            if !previous.contains_key(port) {
                display_watch_new(info);
            }
        }
        for port in previous.keys() {
            if !current.contains_key(port) {
                display_watch_removed(*port);
            }
        }

        previous = current;
        thread::sleep(Duration::from_secs(2));
    }

    println!("\n  Stopped watching.\n");
    Ok(())
}

fn print_help() {
    println!();
    println!("  Port Whisperer - listen to your ports");
    println!();
    println!("  Usage:");
    println!("    ports              Show dev server ports");
    println!("    ports --all        Show all listening ports");
    println!("    ports ps           Show all running dev processes");
    println!("    ports <number>     Detailed info about a specific port");
    println!("    ports kill <n>     Kill by port or PID (-f for force)");
    println!("    ports clean        Kill orphaned/zombie dev servers");
    println!("    ports watch        Monitor port changes in real time");
    println!("    whoisonport <num>  Alias for ports <number>");
    println!();
}

fn parse_port(value: &str) -> Result<u16> {
    let port = value
        .parse::<u16>()
        .map_err(|_| anyhow::anyhow!("Unknown command: {value}"))?;
    if port == 0 {
        bail!("port must be between 1 and 65535");
    }
    Ok(port)
}

fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}
