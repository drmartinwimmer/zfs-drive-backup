use crate::config::{BackupConfig, ConfigError};
use clap::Parser;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur while executing the CLI.
#[derive(Debug, Error)]
pub enum CliError {
    /// Configuration loading or validation error.
    #[error(transparent)]
    Config(#[from] ConfigError),
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

    /// Executes the CLI workflow.
    pub fn run(self) -> Result<(), CliError> {
        let _config = self.load_config()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

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
    fn load_config_reads_custom_path() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut temp = NamedTempFile::new()?;
        writeln!(
            temp,
            r#"{{"datasets": ["tuxedo/home"], "pools": ["backup"]}}"#
        )?;
        let cli = Cli::new(Some(temp.path().to_path_buf()), true);
        let config = cli.load_config()?;
        expect_that!(config.datasets(), elements_are![eq("tuxedo/home")]);
        expect_that!(config.pools(), elements_are![eq("backup")]);
        expect_that!(cli.run(), ok(anything()));
        Ok(())
    }
}
