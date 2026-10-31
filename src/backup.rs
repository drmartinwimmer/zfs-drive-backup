use crate::command::{ClapbackRunner, CommandRunner, HostnameCommand, ZfsCommand};
use crate::config::BackupConfig;
use clapback::CommandError;
use std::collections::HashSet;
use thiserror::Error;

/// Errors that can occur during ZFS dataset replication.
#[derive(Debug, Error)]
pub enum BackupError {
    /// Underlying CLI command failed.
    #[error(transparent)]
    Command(#[from] CommandError),

    /// Hostname command returned an empty or invalid machine name.
    #[error("resolved machine name '{0}' is invalid")]
    InvalidMachineName(String),

    /// Source dataset has no snapshots available to send.
    #[error("source dataset '{dataset}' has no snapshots to send")]
    NoSnapshotsFound {
        /// Name of the source dataset without snapshots.
        dataset: String,
    },

    /// Target dataset already has snapshots, but none match any snapshot on the source dataset.
    #[error(
        "target dataset '{target_dataset}' has existing snapshots but shares no common snapshot with source '{source_dataset}'"
    )]
    NoCommonSnapshot {
        /// Source dataset name.
        source_dataset: String,
        /// Target dataset name on the backup pool.
        target_dataset: String,
    },
}

/// Describes the replication action performed for a single dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicationKind {
    /// Initial full replication stream (`zfs send -R <source>@<latest_snap>`).
    Full {
        /// Latest snapshot short name sent.
        to_snapshot: String,
    },
    /// Incremental replication stream (`zfs send -R -I @<from_snap> <source>@<to_snap>`).
    Incremental {
        /// Base common snapshot short name.
        from_snapshot: String,
        /// Latest snapshot short name sent.
        to_snapshot: String,
    },
    /// Target dataset already contains the latest snapshot from the source dataset.
    UpToDate {
        /// Latest snapshot short name already present on the target.
        latest_snapshot: String,
    },
}

/// Summary of replicating a single configured dataset to the backup pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetReplicationReport {
    source_dataset: String,
    target_dataset: String,
    kind: ReplicationKind,
}

impl DatasetReplicationReport {
    /// Creates a new [`DatasetReplicationReport`].
    #[must_use]
    fn new(source_dataset: String, target_dataset: String, kind: ReplicationKind) -> Self {
        Self {
            source_dataset,
            target_dataset,
            kind,
        }
    }

    /// Returns the source dataset name.
    #[must_use]
    pub fn source_dataset(&self) -> &str {
        &self.source_dataset
    }

    /// Returns the target dataset name on the backup pool.
    #[must_use]
    pub fn target_dataset(&self) -> &str {
        &self.target_dataset
    }

    /// Returns the [`ReplicationKind`] performed for this dataset.
    #[must_use]
    pub fn kind(&self) -> &ReplicationKind {
        &self.kind
    }
}

fn resolve_machine_name_with_runner<R: CommandRunner>(
    runner: &R,
    config: &BackupConfig,
) -> Result<String, BackupError> {
    let raw = match config.machine_name() {
        Some(configured) => configured.to_owned(),
        None => runner.stdout_text(&HostnameCommand::new())?,
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.chars().any(char::is_whitespace) {
        return Err(BackupError::InvalidMachineName(raw));
    }
    Ok(trimmed.to_owned())
}

/// Computes the target dataset path on `backup_pool` for `source_dataset` under `machine_name`.
///
/// Strips the source pool prefix from `source_dataset` so `<source_pool>/<path>` maps to
/// `<backup_pool>/<machine_name>/<path>`. If `source_dataset` has no `/`, it maps to
/// `<backup_pool>/<machine_name>`.
#[must_use]
fn target_dataset_for(backup_pool: &str, machine_name: &str, source_dataset: &str) -> String {
    match source_dataset.split_once('/') {
        Some((_source_pool, relative_path)) if !relative_path.is_empty() => {
            format!("{backup_pool}/{machine_name}/{relative_path}")
        }
        _ => format!("{backup_pool}/{machine_name}"),
    }
}

/// Parses the output of `zfs list -H -o name -t snapshot` into ordered snapshot short names
/// (the portion after `@`).
#[must_use]
fn parse_snapshot_short_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (_dataset, snap_name) = trimmed.split_once('@')?;
            if snap_name.is_empty() {
                None
            } else {
                Some(snap_name.to_owned())
            }
        })
        .collect()
}

/// Replicates a single `source_dataset` to `target_dataset` using `zfs send` and `zfs receive`.
pub fn replicate_dataset(
    source_dataset: &str,
    target_dataset: &str,
) -> Result<DatasetReplicationReport, BackupError> {
    replicate_dataset_with_runner(&ClapbackRunner::new(), source_dataset, target_dataset)
}

pub(crate) fn replicate_dataset_with_runner<R: CommandRunner>(
    runner: &R,
    source_dataset: &str,
    target_dataset: &str,
) -> Result<DatasetReplicationReport, BackupError> {
    let source_snaps_out = runner.stdout_text(&ZfsCommand::list_snapshots(source_dataset))?;
    let source_snaps = parse_snapshot_short_names(&source_snaps_out);
    let latest_snap = source_snaps
        .last()
        .ok_or_else(|| BackupError::NoSnapshotsFound {
            dataset: source_dataset.to_owned(),
        })?
        .clone();

    // Ensure the parent container dataset on the backup pool exists (e.g. `<backup_pool>/<machine_name>`)
    if let Some((parent_dataset, _leaf)) = target_dataset.rsplit_once('/')
        && parent_dataset.contains('/')
        && runner
            .stdout_text(&ZfsCommand::list_dataset(parent_dataset))
            .is_err()
    {
        runner.run(&ZfsCommand::create_container(parent_dataset))?;
    }

    let target_exists = runner
        .stdout_text(&ZfsCommand::list_dataset(target_dataset))
        .is_ok();

    if target_exists {
        let target_snaps_out = runner.stdout_text(&ZfsCommand::list_snapshots(target_dataset))?;
        let target_snaps = parse_snapshot_short_names(&target_snaps_out);

        if !target_snaps.is_empty() {
            let target_snap_set: HashSet<&str> = target_snaps.iter().map(String::as_str).collect();
            let latest_common = source_snaps
                .iter()
                .rev()
                .find(|snap| target_snap_set.contains(snap.as_str()))
                .cloned();

            let Some(common_snap) = latest_common else {
                return Err(BackupError::NoCommonSnapshot {
                    source_dataset: source_dataset.to_owned(),
                    target_dataset: target_dataset.to_owned(),
                });
            };

            if common_snap == latest_snap {
                return Ok(DatasetReplicationReport::new(
                    source_dataset.to_owned(),
                    target_dataset.to_owned(),
                    ReplicationKind::UpToDate {
                        latest_snapshot: latest_snap,
                    },
                ));
            }

            let send_cmd = ZfsCommand::send_incremental(
                format!("@{common_snap}"),
                format!("{source_dataset}@{latest_snap}"),
            );
            let recv_cmd = ZfsCommand::receive(target_dataset);
            runner.run_piped(&send_cmd, &recv_cmd)?;

            return Ok(DatasetReplicationReport::new(
                source_dataset.to_owned(),
                target_dataset.to_owned(),
                ReplicationKind::Incremental {
                    from_snapshot: common_snap,
                    to_snapshot: latest_snap,
                },
            ));
        }
    }

    let send_cmd = ZfsCommand::send_full(format!("{source_dataset}@{latest_snap}"));
    let recv_cmd = ZfsCommand::receive(target_dataset);
    runner.run_piped(&send_cmd, &recv_cmd)?;

    Ok(DatasetReplicationReport::new(
        source_dataset.to_owned(),
        target_dataset.to_owned(),
        ReplicationKind::Full {
            to_snapshot: latest_snap,
        },
    ))
}

/// Replicates all datasets configured in `config` to `backup_pool` under the resolved machine name.
pub fn replicate_all_datasets(
    config: &BackupConfig,
    backup_pool: &str,
) -> Result<Vec<DatasetReplicationReport>, BackupError> {
    replicate_all_datasets_with_runner(&ClapbackRunner::new(), config, backup_pool)
}

pub(crate) fn replicate_all_datasets_with_runner<R: CommandRunner>(
    runner: &R,
    config: &BackupConfig,
    backup_pool: &str,
) -> Result<Vec<DatasetReplicationReport>, BackupError> {
    let machine_name = resolve_machine_name_with_runner(runner, config)?;
    let mut reports = Vec::with_capacity(config.datasets().len());

    for source_dataset in config.datasets() {
        let target_dataset = target_dataset_for(backup_pool, &machine_name, source_dataset);
        let report = replicate_dataset_with_runner(runner, source_dataset, &target_dataset)?;
        reports.push(report);
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clapback::{Command, CommandExt};
    use googletest::prelude::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::error::Error as StdError;
    use std::result::Result as StdResult;

    #[derive(Default)]
    struct ScriptedRunner {
        recorded: RefCell<Vec<String>>,
        stdout_map: HashMap<String, StdResult<String, String>>,
    }

    impl ScriptedRunner {
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

    impl CommandRunner for ScriptedRunner {
        fn stdout_text<C: Command>(&self, cmd: &C) -> Result<String, CommandError> {
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

        fn run<C: Command>(&self, cmd: &C) -> Result<(), CommandError> {
            self.recorded.borrow_mut().push(cmd.display().to_string());
            Ok(())
        }

        fn run_piped<S: Command, R: Command>(
            &self,
            sender: &S,
            receiver: &R,
        ) -> Result<(), CommandError> {
            self.recorded.borrow_mut().push(format!(
                "{} | {}",
                sender.display(),
                receiver.display()
            ));
            Ok(())
        }
    }

    #[googletest::test]
    fn target_dataset_strips_source_pool_prefix() {
        expect_that!(
            target_dataset_for("backup", "workstation", "rpool/home"),
            eq("backup/workstation/home")
        );
        expect_that!(
            target_dataset_for("backup", "workstation", "rpool/var/lib"),
            eq("backup/workstation/var/lib")
        );
        expect_that!(
            target_dataset_for("backup", "workstation", "rpool"),
            eq("backup/workstation")
        );
    }

    #[googletest::test]
    fn resolve_machine_name_prefers_config_and_falls_back_to_hostname()
    -> StdResult<(), Box<dyn StdError>> {
        let runner = ScriptedRunner::new([("hostname", Ok("laptop-host\n"))]);

        let with_override = BackupConfig::new(
            vec!["rpool/home".to_owned()],
            vec!["backup".to_owned()],
            Some("workstation".to_owned()),
        )?;
        expect_that!(
            resolve_machine_name_with_runner(&runner, &with_override)?,
            eq("workstation")
        );
        expect_that!(runner.recorded_commands().is_empty(), is_true());

        let without_override = BackupConfig::new(
            vec!["rpool/home".to_owned()],
            vec!["backup".to_owned()],
            None,
        )?;
        expect_that!(
            resolve_machine_name_with_runner(&runner, &without_override)?,
            eq("laptop-host")
        );
        expect_that!(runner.recorded_commands(), elements_are![eq("hostname")]);
        Ok(())
    }

    #[googletest::test]
    fn replicate_dataset_performs_full_send_when_target_does_not_exist() -> Result<()> {
        let runner = ScriptedRunner::new([
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
                Ok("rpool/home@snap1\nrpool/home@snap2\n"),
            ),
            ("zfs list -H -o name backup/workstation", Err("not found")),
            (
                "zfs list -H -o name backup/workstation/home",
                Err("not found"),
            ),
        ]);

        let report =
            replicate_dataset_with_runner(&runner, "rpool/home", "backup/workstation/home")?;
        verify_that!(report.source_dataset(), eq("rpool/home"))?;
        verify_that!(report.target_dataset(), eq("backup/workstation/home"))?;
        verify_that!(
            report.kind(),
            eq(&ReplicationKind::Full {
                to_snapshot: "snap2".to_owned(),
            })
        )?;
        verify_that!(
            runner.recorded_commands(),
            elements_are![
                eq("zfs list -H -o name -t snapshot -s creation -d 1 rpool/home"),
                eq("zfs list -H -o name backup/workstation"),
                eq("sudo zfs create -p -o mountpoint=none backup/workstation"),
                eq("zfs list -H -o name backup/workstation/home"),
                eq(
                    "sudo zfs send -R rpool/home@snap2 | sudo zfs receive -F -u backup/workstation/home"
                )
            ]
        )
    }

    #[googletest::test]
    fn replicate_dataset_performs_incremental_send_when_common_snapshot_exists() -> Result<()> {
        let runner = ScriptedRunner::new([
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
                Ok("rpool/home@snap1\nrpool/home@snap2\nrpool/home@snap3\n"),
            ),
            (
                "zfs list -H -o name backup/workstation",
                Ok("backup/workstation"),
            ),
            (
                "zfs list -H -o name backup/workstation/home",
                Ok("backup/workstation/home"),
            ),
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 backup/workstation/home",
                Ok("backup/workstation/home@snap1\nbackup/workstation/home@snap2\n"),
            ),
        ]);

        let report =
            replicate_dataset_with_runner(&runner, "rpool/home", "backup/workstation/home")?;
        verify_that!(
            report.kind(),
            eq(&ReplicationKind::Incremental {
                from_snapshot: "snap2".to_owned(),
                to_snapshot: "snap3".to_owned(),
            })
        )?;
        verify_that!(
            runner.recorded_commands(),
            elements_are![
                eq("zfs list -H -o name -t snapshot -s creation -d 1 rpool/home"),
                eq("zfs list -H -o name backup/workstation"),
                eq("zfs list -H -o name backup/workstation/home"),
                eq("zfs list -H -o name -t snapshot -s creation -d 1 backup/workstation/home"),
                eq(
                    "sudo zfs send -R -I @snap2 rpool/home@snap3 | sudo zfs receive -F -u backup/workstation/home"
                )
            ]
        )
    }

    #[googletest::test]
    fn replicate_dataset_skips_send_when_target_already_has_latest_snapshot() -> Result<()> {
        let runner = ScriptedRunner::new([
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
                Ok("rpool/home@snap1\nrpool/home@snap2\n"),
            ),
            (
                "zfs list -H -o name backup/workstation",
                Ok("backup/workstation"),
            ),
            (
                "zfs list -H -o name backup/workstation/home",
                Ok("backup/workstation/home"),
            ),
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 backup/workstation/home",
                Ok("backup/workstation/home@snap1\nbackup/workstation/home@snap2\n"),
            ),
        ]);

        let report =
            replicate_dataset_with_runner(&runner, "rpool/home", "backup/workstation/home")?;
        verify_that!(
            report.kind(),
            eq(&ReplicationKind::UpToDate {
                latest_snapshot: "snap2".to_owned(),
            })
        )
    }

    #[googletest::test]
    fn replicate_dataset_errors_when_source_has_no_snapshots_or_no_common_snapshot() {
        let empty_runner = ScriptedRunner::new([(
            "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
            Ok(""),
        )]);
        let res =
            replicate_dataset_with_runner(&empty_runner, "rpool/home", "backup/workstation/home");
        expect_that!(
            res,
            err(matches_pattern!(BackupError::NoSnapshotsFound { .. }))
        );

        let disjoint_runner = ScriptedRunner::new([
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 rpool/home",
                Ok("rpool/home@new_snap\n"),
            ),
            (
                "zfs list -H -o name backup/workstation",
                Ok("backup/workstation"),
            ),
            (
                "zfs list -H -o name backup/workstation/home",
                Ok("backup/workstation/home"),
            ),
            (
                "zfs list -H -o name -t snapshot -s creation -d 1 backup/workstation/home",
                Ok("backup/workstation/home@unrelated_snap\n"),
            ),
        ]);
        let disjoint_res = replicate_dataset_with_runner(
            &disjoint_runner,
            "rpool/home",
            "backup/workstation/home",
        );
        expect_that!(
            disjoint_res,
            err(matches_pattern!(BackupError::NoCommonSnapshot { .. }))
        );
    }
}
