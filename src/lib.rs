mod cli;
mod cpu_cache;
mod detect;
mod docker;
mod listeners;
mod model;
mod output;
mod scanner;
mod system;
#[cfg(unix)]
mod unix_tcp;
#[cfg(windows)]
mod windows_tcp;

pub fn run(program: &str, args: impl Iterator<Item = String>) {
    if let Err(error) = cli::run(program, args.collect()) {
        eprintln!();
        eprintln!("  Error: {error}");
        eprintln!();
        std::process::exit(1);
    }
}
