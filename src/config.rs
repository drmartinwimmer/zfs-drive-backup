use api_manifest::ConfigFile;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

const APP_QUALIFIER: &str = "";
const APP_ORGANIZATION: &str = "";
const APP_NAME: &str = "zfs-drive-backup";

/// Errors that can occur when locating, reading, or validating the backup configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Could not determine the user's platform configuration directory.
    #[error("unable to determine user configuration directory")]
    NoConfigDir,

    /// Failed to read the configuration file from disk.
    #[error("failed to read configuration file '{}': {source}", path.display())]
    Io {
        /// Path to the configuration file.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },

    /// Failed to parse the configuration file as JSON.
    #[error("failed to parse configuration JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// No datasets were configured for backup.
    #[error("configuration must specify at least one dataset in 'datasets'")]
    EmptyDatasets,

    /// A configured dataset name is invalid.
    #[error("invalid dataset name '{0}': dataset name must not be empty or contain whitespace")]
    InvalidDataset(String),

    /// No candidate backup pools were configured.
    #[error("configuration must specify at least one backup pool in 'pools'")]
    EmptyPools,

    /// A configured backup pool name is invalid.
    #[error(
        "invalid backup pool name '{0}': pool name must not be empty, contain '/', or contain whitespace"
    )]
    InvalidPool(String),

    /// A configured machine name override is invalid.
    #[error(
        "invalid machine name '{0}': machine name must not be empty, contain '/', or contain whitespace"
    )]
    InvalidMachineName(String),
}

/// User configuration specifying datasets to back up and candidate external backup pools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ConfigFile)]
#[config_file("config.json")]
pub struct BackupConfig {
    /// ZFS datasets to back up.
    datasets: Vec<String>,
    /// Candidate external ZFS backup pool names to discover and mount.
    pools: Vec<String>,
    /// Optional machine name override for the target dataset prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    machine_name: Option<String>,
}

impl BackupConfig {
    /// Creates and validates a new [`BackupConfig`].
    pub fn new(
        datasets: Vec<String>,
        pools: Vec<String>,
        machine_name: Option<String>,
    ) -> Result<Self, ConfigError> {
        let config = Self {
            datasets,
            pools,
            machine_name,
        };
        config.validate()?;
        Ok(config)
    }

    /// Parses and validates a [`BackupConfig`] from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Loads and validates a [`BackupConfig`] from a JSON file at `path`.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let contents = fs::read_to_string(path_ref).map_err(|source| ConfigError::Io {
            path: path_ref.to_path_buf(),
            source,
        })?;
        Self::from_json_str(&contents)
    }

    /// Loads and validates a [`BackupConfig`] from the default user configuration path.
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = default_config_path()?;
        Self::load_from_path(path)
    }

    /// Validates that datasets, pools, and optional machine name are well-formed.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.datasets.is_empty() {
            return Err(ConfigError::EmptyDatasets);
        }
        for dataset in &self.datasets {
            if dataset.is_empty()
                || dataset.chars().any(char::is_whitespace)
                || dataset.starts_with('/')
                || dataset.ends_with('/')
                || dataset.contains("//")
            {
                return Err(ConfigError::InvalidDataset(dataset.clone()));
            }
        }

        if self.pools.is_empty() {
            return Err(ConfigError::EmptyPools);
        }
        for pool in &self.pools {
            if pool.is_empty() || pool.contains('/') || pool.chars().any(char::is_whitespace) {
                return Err(ConfigError::InvalidPool(pool.clone()));
            }
        }

        if let Some(machine) = &self.machine_name
            && (machine.is_empty()
                || machine.contains('/')
                || machine.chars().any(char::is_whitespace))
        {
            return Err(ConfigError::InvalidMachineName(machine.clone()));
        }

        Ok(())
    }

    /// Returns the list of ZFS datasets configured to be backed up.
    #[must_use]
    pub fn datasets(&self) -> &[String] {
        &self.datasets
    }

    /// Returns the list of candidate external ZFS backup pool names.
    #[must_use]
    pub fn pools(&self) -> &[String] {
        &self.pools
    }

    /// Returns the optional machine name override.
    #[must_use]
    pub fn machine_name(&self) -> Option<&str> {
        self.machine_name.as_deref()
    }
}

/// Returns the default configuration file path in the user's configuration directory
/// (e.g. `~/.config/zfs-drive-backup/config.json` on Linux).
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    let proj_dirs = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
        .ok_or(ConfigError::NoConfigDir)?;
    Ok(proj_dirs
        .config_dir()
        .join(BackupConfig::config_file_path()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::error::Error as StdError;
    use std::io::Write;
    use std::result::Result as StdResult;
    use tempfile::NamedTempFile;

    #[googletest::test]
    fn default_config_path_ends_with_app_and_config_json() -> Result<()> {
        let path = default_config_path()?;
        verify_that!(path.ends_with("zfs-drive-backup/config.json"), is_true())
    }

    #[googletest::test]
    fn parse_valid_config_with_pools_and_machine_name() -> Result<()> {
        let json = r#"{
            "datasets": ["tuxedo/home", "tuxedo/var"],
            "pools": ["backup-usb", "offsite-pool"],
            "machine_name": "tuxedo"
        }"#;
        let config = BackupConfig::from_json_str(json)?;
        verify_that!(
            config.datasets(),
            elements_are![eq("tuxedo/home"), eq("tuxedo/var")]
        )?;
        verify_that!(
            config.pools(),
            elements_are![eq("backup-usb"), eq("offsite-pool")]
        )?;
        verify_that!(config.machine_name(), some(eq("tuxedo")))
    }

    #[googletest::test]
    fn parse_valid_config_without_machine_name() -> Result<()> {
        let json = r#"{
            "datasets": ["nano/root", "nano/home"],
            "pools": ["extbackup"]
        }"#;
        let config = BackupConfig::from_json_str(json)?;
        verify_that!(
            config.datasets(),
            elements_are![eq("nano/root"), eq("nano/home")]
        )?;
        verify_that!(config.pools(), elements_are![eq("extbackup")])?;
        verify_that!(config.machine_name(), none())
    }

    #[googletest::test]
    fn load_from_path_reads_and_validates_file() -> StdResult<(), Box<dyn StdError>> {
        let mut temp = NamedTempFile::new()?;
        writeln!(
            temp,
            r#"{{"datasets": ["tuxedo/home"], "pools": ["backup"]}}"#
        )?;
        let config = BackupConfig::load_from_path(temp.path())?;
        expect_that!(config.datasets(), elements_are![eq("tuxedo/home")]);
        expect_that!(config.pools(), elements_are![eq("backup")]);
        Ok(())
    }

    #[googletest::test]
    fn load_from_nonexistent_path_returns_io_error() {
        let res = BackupConfig::load_from_path("/nonexistent/zfs-drive-backup/config.json");
        expect_that!(res, err(matches_pattern!(ConfigError::Io { .. })));
    }

    #[googletest::test]
    fn reject_empty_datasets_or_pools() {
        let empty_datasets = BackupConfig::new(vec![], vec!["backup".to_owned()], None);
        expect_that!(
            empty_datasets,
            err(matches_pattern!(ConfigError::EmptyDatasets))
        );

        let empty_pools = BackupConfig::new(vec!["tuxedo/home".to_owned()], vec![], None);
        expect_that!(empty_pools, err(matches_pattern!(ConfigError::EmptyPools)));
    }

    #[googletest::test]
    fn reject_invalid_dataset_pool_or_machine_names() {
        for bad_dataset in [
            "",
            "tuxedo/home with space",
            "/leading",
            "trailing/",
            "a//b",
        ] {
            let res = BackupConfig::new(
                vec![bad_dataset.to_owned()],
                vec!["backup".to_owned()],
                None,
            );
            expect_that!(res, err(matches_pattern!(ConfigError::InvalidDataset(_))));
        }

        for bad_pool in ["", "pool/sub", "pool space"] {
            let res = BackupConfig::new(
                vec!["tuxedo/home".to_owned()],
                vec![bad_pool.to_owned()],
                None,
            );
            expect_that!(res, err(matches_pattern!(ConfigError::InvalidPool(_))));
        }

        for bad_machine in ["", "machine/sub", "machine space"] {
            let res = BackupConfig::new(
                vec!["tuxedo/home".to_owned()],
                vec!["backup".to_owned()],
                Some(bad_machine.to_owned()),
            );
            expect_that!(
                res,
                err(matches_pattern!(ConfigError::InvalidMachineName(_)))
            );
        }
    }
}
