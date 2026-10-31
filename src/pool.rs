use crate::command::{ClapbackRunner, CommandRunner, ZpoolCommand};
use clapback::CommandError;
use thiserror::Error;

/// Errors that can occur when discovering, importing, or exporting a ZFS backup pool.
#[derive(Debug, Error)]
pub enum PoolError {
    /// Underlying CLI command execution failed.
    #[error(transparent)]
    Command(#[from] CommandError),

    /// None of the configured backup pools is currently imported or available for import.
    #[error(
        "none of the configured backup pools ({configured:?}) was found (imported: {imported:?}, importable: {importable:?})"
    )]
    NoBackupPoolFound {
        /// Pool names specified in the configuration.
        configured: Vec<String>,
        /// Pools currently imported on the system.
        imported: Vec<String>,
        /// Pools discovered via `zpool import`.
        importable: Vec<String>,
    },
}

/// Parses the output of `zpool list -H -o name` into a list of imported pool names.
#[must_use]
fn parse_pool_list_output(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Parses the output of `zpool import` into a list of importable pool names.
#[must_use]
fn parse_zpool_import_output(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("pool:")?;
            let pool_name = rest.trim();
            if pool_name.is_empty() {
                None
            } else {
                Some(pool_name.to_owned())
            }
        })
        .collect()
}

/// Discovers a configured backup pool and imports/mounts it if not already imported.
///
/// 1. Checks currently imported pools via `zpool list -H -o name`.
/// 2. If none of `configured_pools` is imported, discovers available pools via `sudo zpool import`
///    and imports the first matching configured pool with `sudo zpool import -l <pool>`.
pub fn discover_and_mount_pool(configured_pools: &[String]) -> Result<String, PoolError> {
    discover_and_mount_pool_with_runner(&ClapbackRunner::new(), configured_pools)
}

pub(crate) fn discover_and_mount_pool_with_runner<R: CommandRunner>(
    runner: &R,
    configured_pools: &[String],
) -> Result<String, PoolError> {
    let list_stdout = runner.stdout_text(&ZpoolCommand::list_names())?;
    let imported = parse_pool_list_output(&list_stdout);

    if let Some(already_imported) = configured_pools
        .iter()
        .find(|candidate| imported.iter().any(|imp| imp == *candidate))
    {
        return Ok(already_imported.clone());
    }

    let import_stdout = runner.stdout_text(&ZpoolCommand::import_discover())?;
    let importable = parse_zpool_import_output(&import_stdout);

    if let Some(pool_to_import) = configured_pools
        .iter()
        .find(|candidate| importable.iter().any(|avail| avail == *candidate))
    {
        runner.run(&ZpoolCommand::import_pool(pool_to_import.clone()))?;
        return Ok(pool_to_import.clone());
    }

    Err(PoolError::NoBackupPoolFound {
        configured: configured_pools.to_vec(),
        imported,
        importable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clapback::{Command, CommandExt};
    use googletest::prelude::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct MockRunner {
        recorded: RefCell<Vec<String>>,
        stdout_responses: RefCell<VecDeque<Result<String, CommandError>>>,
    }

    impl MockRunner {
        fn with_stdout_responses(responses: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                recorded: RefCell::new(Vec::new()),
                stdout_responses: RefCell::new(
                    responses.into_iter().map(|s| Ok(s.to_owned())).collect(),
                ),
            }
        }

        fn recorded_commands(&self) -> Vec<String> {
            self.recorded.borrow().clone()
        }
    }

    impl CommandRunner for MockRunner {
        fn stdout_text<C: Command>(&self, cmd: &C) -> Result<String, CommandError> {
            self.recorded.borrow_mut().push(cmd.display().to_string());
            self.stdout_responses
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Ok(String::new()))
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
    fn parse_zpool_import_extracts_pool_names() {
        let sample = "\
   pool: backup-usb
     id: 1234567890123456789
  state: ONLINE
 action: The pool can be imported using its name or numeric identifier.
 config:

        backup-usb  ONLINE
          sda1      ONLINE

   pool: archive-drive
     id: 9876543210987654321
  state: ONLINE
";
        let pools = parse_zpool_import_output(sample);
        expect_that!(pools, elements_are![eq("backup-usb"), eq("archive-drive")]);
    }

    #[googletest::test]
    fn discover_and_mount_pool_returns_already_imported_pool_without_importing() -> Result<()> {
        let runner = MockRunner::with_stdout_responses(["tuxedo\nbackup-usb\n"]);
        let configured = vec!["offsite".to_owned(), "backup-usb".to_owned()];

        let pool = discover_and_mount_pool_with_runner(&runner, &configured)?;
        verify_that!(pool, eq("backup-usb"))?;
        verify_that!(
            runner.recorded_commands(),
            elements_are![eq("zpool list -H -o name")]
        )
    }

    #[googletest::test]
    fn discover_and_mount_pool_imports_discovered_pool_with_load_keys() -> Result<()> {
        let import_output = "\
   pool: backup-usb
     id: 12345
  state: ONLINE
";
        let runner = MockRunner::with_stdout_responses(["tuxedo\n", import_output]);
        let configured = vec!["offsite".to_owned(), "backup-usb".to_owned()];

        let pool = discover_and_mount_pool_with_runner(&runner, &configured)?;
        verify_that!(pool, eq("backup-usb"))?;
        verify_that!(
            runner.recorded_commands(),
            elements_are![
                eq("zpool list -H -o name"),
                eq("sudo zpool import"),
                eq("sudo zpool import -l backup-usb")
            ]
        )
    }

    #[googletest::test]
    fn discover_and_mount_pool_errors_when_no_configured_pool_is_available() {
        let runner =
            MockRunner::with_stdout_responses(["tuxedo\n", "no pools available to import"]);
        let configured = vec!["backup-usb".to_owned()];

        let res = discover_and_mount_pool_with_runner(&runner, &configured);
        expect_that!(
            res,
            err(matches_pattern!(PoolError::NoBackupPoolFound { .. }))
        );
    }
}
