use crate::backup::{
    BackupError, DatasetReplicationReport, ReplicationKind, replicate_all_datasets_with_runner,
};
use crate::command::{ClapbackRunner, CommandRunner};
use crate::config::{BackupConfig, ConfigError};
use crate::pool::{PoolError, discover_and_mount_pool_with_runner, export_pool_with_runner};
use clap::Parser;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur while executing the CLI.
#[derive(Debug, Error)]
pub enum CliError {
    /// Configuration loading or validation error.
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Pool discovery, import, or export error.
    #[error(transparent)]
    Pool(#[from] PoolError),

    /// Dataset snapshot replication error.
    #[error(transparent)]
    Backup(#[from] BackupError),
}

/// Summary of a completed backup run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupSummary {
    backup_pool: String,
    datasets: Vec<DatasetReplicationReport>,
    exported: bool,
}

impl BackupSummary {
    /// Creates a new [`BackupSummary`].
    #[must_use]
    fn new(backup_pool: String, datasets: Vec<DatasetReplicationReport>, exported: bool) -> Self {
        Self {
            backup_pool,
            datasets,
            exported,
        }
    }

    /// Returns the name of the external backup pool used.
    #[must_use]
    pub fn backup_pool(&self) -> &str {
        &self.backup_pool
    }

    /// Returns the per-dataset replication reports.
    #[must_use]
    pub fn datasets(&self) -> &[DatasetReplicationReport] {
        &self.datasets
    }

    /// Returns `true` if the backup pool was exported at the end of the run.
    #[must_use]
    pub fn exported(&self) -> bool {
        self.exported
    }
}

/// Helper tool for easily backing up ZFS datasets to external hard drives using zfs send
#[derive(Debug, Clone, Default, PartialEq, Eq, Parser)]
#[command(name = "zfs-drive-backup", version, about)]
pub struct Cli {
    /// Path to the JSON configuration file (defaults to ~/.config/zfs-drive-backup/config.json)
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Export the external ZFS backup pool after completing the backup
    #[arg(short, long)]
    export: bool,
}

impl Cli {
    /// Constructs a new [`Cli`] instance with the given options.
    #[must_use]
    pub fn new(config: Option<PathBuf>, export: bool) -> Self {
        Self { config, export }
    }

    /// Returns the custom configuration file path if provided.
    #[must_use]
    pub fn config_path(&self) -> Option<&Path> {
        self.config.as_deref()
    }

    /// Returns `true` if `--export` was specified.
    #[must_use]
    pub fn export(&self) -> bool {
        self.export
    }

    /// Loads and validates the [`BackupConfig`] from either `--config` or the default user config directory.
    pub fn load_config(&self) -> Result<BackupConfig, ConfigError> {
        match &self.config {
            Some(path) => BackupConfig::load_from_path(path),
            None => BackupConfig::load_default(),
        }
    }

    pub(crate) fn run_with_runner<R: CommandRunner>(
        &self,
        runner: &R,
    ) -> Result<BackupSummary, CliError> {
        let config = self.load_config()?;
        let backup_pool = discover_and_mount_pool_with_runner(runner, config.pools())?;
        let datasets = replicate_all_datasets_with_runner(runner, &config, &backup_pool)?;

        if self.export {
            export_pool_with_runner(runner, &backup_pool)?;
        }

        Ok(BackupSummary::new(backup_pool, datasets, self.export))
    }

    /// Executes the CLI workflow.
    pub fn run(self) -> Result<(), CliError> {
        let summary = self.run_with_runner(&ClapbackRunner::new())?;
        for report in summary.datasets() {
            match report.kind() {
                ReplicationKind::Full { to_snapshot } => {
                    println!(
                        "Backed up {} -> {} (full replication @{})",
                        report.source_dataset(),
                        report.target_dataset(),
                        to_snapshot
                    );
                }
                ReplicationKind::Incremental {
                    from_snapshot,
                    to_snapshot,
                } => {
                    println!(
                        "Backed up {} -> {} (incremental @{} -> @{})",
                        report.source_dataset(),
                        report.target_dataset(),
                        from_snapshot,
                        to_snapshot
                    );
                }
                ReplicationKind::UpToDate { latest_snapshot } => {
                    println!(
                        "Skipped {} -> {} (already up to date at @{})",
                        report.source_dataset(),
                        report.target_dataset(),
                        latest_snapshot
                    );
                }
            }
        }
        if summary.exported() {
            println!("Exported backup pool '{}'.", summary.backup_pool());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clapback::{Command, CommandError, CommandExt};
    use googletest::prelude::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::error::Error as StdError;
    use std::io::Write;
    use std::result::Result as StdResult;
    use tempfile::NamedTempFile;

    #[derive(Default)]
    struct WorkflowRunner {
        recorded: RefCell<Vec<String>>,
        stdout_map: HashMap<String, StdResult<String, String>>,
    }

    impl WorkflowRunner {
        fn new(
            entries: impl IntoIterator<Item = (&'static str, StdResult<&'static str, &'static str>)>,
        ) -> Self {
            let stdout_map = entries
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.to_owned(),
                        v.map(ToOwned::to_owned).map_err(ToOwned::to_owned),
                    )
                })
                .collect();
            Self {
                recorded: RefCell::new(Vec::new()),
                stdout_map,
            }
        }

        fn recorded_commands(&self) -> Vec<String> {
            self.recorded.borrow().clone()
        }
    }

    impl CommandRunner for WorkflowRunner {
        fn stdout_text<C: Command>(&self, cmd: &C) -> StdResult<String, CommandError> {
            let rendered = cmd.display().to_string();
            self.recorded.borrow_mut().push(rendered.clone());
            match self.stdout_map.get(&rendered) {
                Some(Ok(out)) => Ok(out.clone()),
                Some(Err(msg)) => Err(CommandError::ProgramNotFound(msg.clone())),
                None => Err(CommandError::ProgramNotFound(format!(
                    "unmocked command: {rendered}"
                ))),
            }
        }

        fn run<C: Command>(&self, cmd: &C) -> StdResult<(), CommandError> {
            self.recorded.borrow_mut().push(cmd.display().to_string());
            Ok(())
        }

        fn run_piped<S: Command, R: Command>(
            &self,
            sender: &S,
            receiver: &R,
        ) -> StdResult<(), CommandError> {
            self.recorded.borrow_mut().push(format!(
                "{} | {}",
                sender.display(),
                receiver.display()
            ));
            Ok(())
        }
    }

    #[googletest::test]
    fn parse_cli_defaults() -> Result<()> {
        let cli = Cli::try_parse_from(["zfs-drive-backup"])?;
        verify_that!(cli.config_path(), none())?;
        verify_that!(cli.export(), is_false())
    }

    #[googletest::test]
    fn parse_cli_with_export_and_config_flags() -> Result<()> {
        let cli = Cli::try_parse_from([
            "zfs-drive-backup",
            "--export",
            "--config",
            "/tmp/custom-config.json",
        ])?;
        verify_that!(cli.export(), is_true())?;
        verify_that!(
            cli.config_path(),
            some(eq(Path::new("/tmp/custom-config.json")))
        )
    }

    #[googletest::test]
    fn load_config_reads_custom_path() -> StdResult<(), Box<dyn StdError>> {
        let mut temp = NamedTempFile::new()?;
        writeln!(
            temp,
            r#"{{"datasets": ["rpool/home"], "pools": ["backup"]}}"#
        )?;
        let cli = Cli::new(Some(temp.path().to_path_buf()), true);
        let config = cli.load_config()?;
        expect_that!(config.datasets(), elements_are![eq("rpool/home")]);
        expect_that!(config.pools(), elements_are![eq("backup")]);
        Ok(())
    }

    #[googletest::test]
    fn run_with_runner_discovers_pool_replicates_and_exports_when_flag_is_set()
    -> StdResult<(), Box<dyn StdError>> {
        let mut temp = NamedTempFile::new()?;
        writeln!(
            temp,
            r#"{{"datasets": ["rpool/home"], "pools": ["backup-usb"]}}"#
        )?;

        let runner = WorkflowRunner::new([
            ("zpool list -H -o name", Ok("rpool\n")),
            ("sudo zpool import", Ok("   pool: backup-usb\n")),
            ("hostname", Ok("workstation\n")),
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
                Ok("rpool/home@snap1\nrpool/home@snap2\n"),
            ),
            (
                "zfs list -H -o name backup-usb/workstation",
                Err("not found"),
            ),
            (
                "zfs list -H -o name backup-usb/workstation/home",
                Err("not found"),
            ),
        ]);

        let cli = Cli::new(Some(temp.path().to_path_buf()), true);
        let summary = cli.run_with_runner(&runner)?;
        expect_that!(summary.backup_pool(), eq("backup-usb"));
        expect_that!(summary.exported(), is_true());
        expect_that!(summary.datasets().len(), eq(1));
        expect_that!(
            runner.recorded_commands(),
            elements_are![
                eq("zpool list -H -o name"),
                eq("sudo zpool import"),
                eq("sudo zpool import -l backup-usb"),
                eq("hostname"),
                eq("zfs list -H -o name -t snapshot -s creation -d 1 rpool/home"),
                eq("zfs list -H -o name backup-usb/workstation"),
                eq("sudo zfs create -p -o mountpoint=none backup-usb/workstation"),
                eq("zfs list -H -o name backup-usb/workstation/home"),
                eq(
                    "sudo zfs send -R rpool/home@snap2 | sudo zfs receive -F -u backup-usb/workstation/home"
                ),
                eq("sudo zpool export backup-usb")
            ]
        );
        Ok(())
    }

    #[googletest::test]
    fn run_with_runner_omits_export_when_flag_is_false() -> StdResult<(), Box<dyn StdError>> {
        let mut temp = NamedTempFile::new()?;
        writeln!(
            temp,
            r#"{{"datasets": ["rpool/home"], "pools": ["backup-usb"], "machine_name": "workstation"}}"#
        )?;

        let runner = WorkflowRunner::new([
            ("zpool list -H -o name", Ok("rpool\nbackup-usb\n")),
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
                Ok("rpool/home@snap1\n"),
            ),
            (
                "zfs list -H -o name backup-usb/workstation",
                Ok("backup-usb/workstation"),
            ),
            (
                "zfs list -H -o name backup-usb/workstation/home",
                Ok("backup-usb/workstation/home"),
            ),
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 backup-usb/workstation/home",
                Ok("backup-usb/workstation/home@snap1\n"),
            ),
        ]);

        let cli = Cli::new(Some(temp.path().to_path_buf()), false);
        let summary = cli.run_with_runner(&runner)?;
        expect_that!(summary.backup_pool(), eq("backup-usb"));
        expect_that!(summary.exported(), is_false());
        expect_that!(
            runner.recorded_commands(),
            elements_are![
                eq("zpool list -H -o name"),
                eq("zfs list -H -o name -t snapshot -s creation -d 1 rpool/home"),
                eq("zfs list -H -o name backup-usb/workstation"),
                eq("zfs list -H -o name backup-usb/workstation/home"),
                eq("zfs list -H -o name -t snapshot -s creation -d 1 backup-usb/workstation/home")
            ]
        );
        Ok(())
    }
}
