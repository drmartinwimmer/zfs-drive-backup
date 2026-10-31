use clap::Parser;
use std::process::ExitCode;
use zfs_drive_backup::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(err) = cli.run() {
        eprintln!("Error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
