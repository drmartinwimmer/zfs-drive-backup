use clapback::{Command, CommandError, CommandExt, Subcommand as ClapbackSubcommand};

/// Wrapper command that executes an inner [`Command`] via `sudo`.
#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "sudo")]
pub(crate) struct SudoCommand<C: Command> {
    #[command(wrap)]
    command: C,
}

impl<C: Command> SudoCommand<C> {
    /// Wraps `command` inside `sudo`.
    #[must_use]
    pub(crate) fn new(command: C) -> Self {
        Self { command }
    }
}

/// Subcommands supported by `zpool`.
#[derive(ClapbackSubcommand, Debug, Clone, PartialEq, Eq)]
pub(crate) enum ZpoolSubcommand {
    /// List imported storage pools (`zpool list`).
    List {
        /// Scripted mode: omit headers and separate fields with a single tab (`-H`).
        #[arg(short = 'H')]
        scripted: bool,
        /// Comma-separated list of properties to display (`-o`).
        #[arg(short = 'o')]
        output: Option<String>,
    },
    /// Discover or import ZFS storage pools (`zpool import`).
    Import {
        /// Load encryption keys for encrypted datasets while importing (`-l`).
        #[arg(short = 'l')]
        load_keys: bool,
        /// Target pool name to import, or `None` to list importable pools.
        pool: Option<String>,
    },
}

/// CLI command builder for `zpool` invocations.
#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "zpool")]
pub(crate) struct ZpoolCommand {
    #[command(subcommand)]
    subcommand: ZpoolSubcommand,
}

impl ZpoolCommand {
    /// Creates a `zpool` command with the given subcommand.
    #[must_use]
    pub(crate) fn new(subcommand: ZpoolSubcommand) -> Self {
        Self { subcommand }
    }

    /// Builds `zpool list -H -o name` to list imported pool names.
    #[must_use]
    pub(crate) fn list_names() -> Self {
        Self::new(ZpoolSubcommand::List {
            scripted: true,
            output: Some("name".to_owned()),
        })
    }

    /// Builds `sudo zpool import` to discover available pools to import.
    #[must_use]
    pub(crate) fn import_discover() -> SudoCommand<Self> {
        Self::new(ZpoolSubcommand::Import {
            load_keys: false,
            pool: None,
        })
        .wrap_with(SudoCommand::new)
    }

    /// Builds `sudo zpool import -l <pool>` to import and load keys for `pool`.
    #[must_use]
    pub(crate) fn import_pool(pool: impl Into<String>) -> SudoCommand<Self> {
        Self::new(ZpoolSubcommand::Import {
            load_keys: true,
            pool: Some(pool.into()),
        })
        .wrap_with(SudoCommand::new)
    }
}

/// Abstraction for executing [`clapback::Command`] instances, enabling deterministic unit testing.
pub(crate) trait CommandRunner {
    /// Executes `cmd` and returns its trimmed UTF-8 standard output.
    fn stdout_text<C: Command>(&self, cmd: &C) -> Result<String, CommandError>;

    /// Executes `cmd` synchronously with inherited standard I/O.
    fn run<C: Command>(&self, cmd: &C) -> Result<(), CommandError>;
}

/// Default [`CommandRunner`] that executes commands via [`clapback`].
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ClapbackRunner;

impl ClapbackRunner {
    /// Creates a new [`ClapbackRunner`].
    #[must_use]
    pub(crate) fn new() -> Self {
        Self
    }
}

impl CommandRunner for ClapbackRunner {
    fn stdout_text<C: Command>(&self, cmd: &C) -> Result<String, CommandError> {
        CommandExt::stdout_text(cmd)
    }

    fn run<C: Command>(&self, cmd: &C) -> Result<(), CommandError> {
        CommandExt::run(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[derive(Command, Debug, Clone, PartialEq, Eq)]
    #[command(program = "sh", arg = "-c")]
    struct ShCommand {
        script: String,
    }

    #[googletest::test]
    fn zpool_commands_serialize_expected_cli_strings() {
        expect_that!(
            ZpoolCommand::list_names().to_string(),
            eq("zpool list -H -o name")
        );
        expect_that!(
            ZpoolCommand::import_discover().to_string(),
            eq("sudo zpool import")
        );
        expect_that!(
            ZpoolCommand::import_pool("backup").to_string(),
            eq("sudo zpool import -l backup")
        );
    }

    #[googletest::test]
    fn clapback_runner_executes_stdout_command() -> Result<()> {
        let runner = ClapbackRunner::new();
        let out = runner.stdout_text(&ShCommand {
            script: "printf 'hello-zfs\\n'".to_owned(),
        })?;
        verify_that!(out, eq("hello-zfs"))
    }
}
