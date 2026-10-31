use clapback::{Command, CommandError, CommandExt, Subcommand as ClapbackSubcommand};
use std::io;
use std::process::Stdio;

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

/// CLI command that invokes `hostname` to determine the local machine name.
#[derive(Command, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[command(program = "hostname")]
pub(crate) struct HostnameCommand;

impl HostnameCommand {
    /// Creates a new `hostname` command.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self
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
    /// Export an imported ZFS storage pool (`zpool export <pool>`).
    Export {
        /// Name of the storage pool to export.
        pool: String,
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

    /// Builds `sudo zpool export <pool>` to export `pool`.
    #[must_use]
    pub(crate) fn export_pool(pool: impl Into<String>) -> SudoCommand<Self> {
        Self::new(ZpoolSubcommand::Export { pool: pool.into() }).wrap_with(SudoCommand::new)
    }
}

/// Subcommands supported by `zfs`.
#[derive(ClapbackSubcommand, Debug, Clone, PartialEq, Eq)]
pub(crate) enum ZfsSubcommand {
    /// List datasets or snapshots (`zfs list`).
    List {
        /// Scripted mode: omit headers and separate fields with a single tab (`-H`).
        #[arg(short = 'H')]
        scripted: bool,
        /// Comma-separated list of properties to display (`-o`).
        #[arg(short = 'o')]
        output: Option<String>,
        /// Dataset type filter, e.g. `"snapshot"` (`-t`).
        #[arg(short = 't')]
        dataset_type: Option<String>,
        /// Sort ascending by property, e.g. `"creation"` (`-s`).
        #[arg(short = 's')]
        sort: Option<String>,
        /// Recursively display children up to `depth` (`-d`).
        #[arg(short = 'd')]
        depth: Option<u32>,
        /// Target dataset name.
        dataset: Option<String>,
    },
    /// Create a new ZFS filesystem dataset (`zfs create`).
    Create {
        /// Create all non-existing parent datasets (`-p`).
        #[arg(short = 'p')]
        parents: bool,
        /// Set dataset property at creation time (`-o property=value`).
        #[arg(short = 'o')]
        option: Option<String>,
        /// Dataset name to create.
        dataset: String,
    },
    /// Generate a backup stream from a ZFS snapshot (`zfs send`).
    Send {
        /// Generate a replication stream package (`-R`).
        #[arg(short = 'R')]
        replicate: bool,
        /// Generate an incremental stream including all intermediary snapshots from `incremental_from` (`-I`).
        #[arg(short = 'I')]
        incremental_from: Option<String>,
        /// Target snapshot (`dataset@snap`) to send.
        snapshot: String,
    },
    /// Receive a ZFS backup stream into a target dataset (`zfs receive`).
    Receive {
        /// Roll back the target filesystem to the most recent snapshot before receiving (`-F`).
        #[arg(short = 'F')]
        force_rollback: bool,
        /// Do not mount the received filesystem (`-u`).
        #[arg(short = 'u')]
        nomount: bool,
        /// Target dataset path to receive into.
        target: String,
    },
}

/// CLI command builder for `zfs` invocations.
#[derive(Command, Debug, Clone, PartialEq, Eq)]
#[command(program = "zfs")]
pub(crate) struct ZfsCommand {
    #[command(subcommand)]
    subcommand: ZfsSubcommand,
}

impl ZfsCommand {
    /// Creates a `zfs` command with the given subcommand.
    #[must_use]
    pub(crate) fn new(subcommand: ZfsSubcommand) -> Self {
        Self { subcommand }
    }

    /// Builds `zfs list -H -o name <dataset>` to verify whether a dataset exists.
    #[must_use]
    pub(crate) fn list_dataset(dataset: impl Into<String>) -> Self {
        Self::new(ZfsSubcommand::List {
            scripted: true,
            output: Some("name".to_owned()),
            dataset_type: None,
            sort: None,
            depth: None,
            dataset: Some(dataset.into()),
        })
    }

    /// Builds `zfs list -H -o name -t snapshot -s creation -d 1 <dataset>` to list direct snapshots
    /// of `dataset` ordered by creation time.
    #[must_use]
    pub(crate) fn list_snapshots(dataset: impl Into<String>) -> Self {
        Self::new(ZfsSubcommand::List {
            scripted: true,
            output: Some("name".to_owned()),
            dataset_type: Some("snapshot".to_owned()),
            sort: Some("creation".to_owned()),
            depth: Some(1),
            dataset: Some(dataset.into()),
        })
    }

    /// Builds `sudo zfs create -p -o mountpoint=none <dataset>` to ensure a parent container dataset exists.
    #[must_use]
    pub(crate) fn create_container(dataset: impl Into<String>) -> SudoCommand<Self> {
        Self::new(ZfsSubcommand::Create {
            parents: true,
            option: Some("mountpoint=none".to_owned()),
            dataset: dataset.into(),
        })
        .wrap_with(SudoCommand::new)
    }

    /// Builds `sudo zfs send -R <snapshot>` for a full replication stream.
    #[must_use]
    pub(crate) fn send_full(snapshot: impl Into<String>) -> SudoCommand<Self> {
        Self::new(ZfsSubcommand::Send {
            replicate: true,
            incremental_from: None,
            snapshot: snapshot.into(),
        })
        .wrap_with(SudoCommand::new)
    }

    /// Builds `sudo zfs send -R -I <from_snapshot> <to_snapshot>` for an incremental replication stream.
    #[must_use]
    pub(crate) fn send_incremental(
        from_snapshot: impl Into<String>,
        to_snapshot: impl Into<String>,
    ) -> SudoCommand<Self> {
        Self::new(ZfsSubcommand::Send {
            replicate: true,
            incremental_from: Some(from_snapshot.into()),
            snapshot: to_snapshot.into(),
        })
        .wrap_with(SudoCommand::new)
    }

    /// Builds `sudo zfs receive -F -u <target>` to receive a replication stream without mounting over host paths.
    #[must_use]
    pub(crate) fn receive(target: impl Into<String>) -> SudoCommand<Self> {
        Self::new(ZfsSubcommand::Receive {
            force_rollback: true,
            nomount: true,
            target: target.into(),
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

    /// Executes `sender` piped into `receiver` (`sender | receiver`).
    fn run_piped<S: Command, R: Command>(
        &self,
        sender: &S,
        receiver: &R,
    ) -> Result<(), CommandError>;
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

    fn run_piped<S: Command, R: Command>(
        &self,
        sender: &S,
        receiver: &R,
    ) -> Result<(), CommandError> {
        let sender_name = sender.program().to_string_lossy().into_owned();
        let receiver_name = receiver.program().to_string_lossy().into_owned();

        let mut send_proc = sender.to_std_command();
        send_proc
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut send_child = send_proc
            .spawn()
            .map_err(|err| CommandError::from_spawn_io(err, &sender_name))?;

        let send_stdout = send_child.stdout.take().ok_or_else(|| {
            CommandError::Io(io::Error::other("failed to capture stdout of send command"))
        })?;

        let mut recv_proc = receiver.to_std_command();
        recv_proc
            .stdin(Stdio::from(send_stdout))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let mut recv_child = match recv_proc.spawn() {
            Ok(child) => child,
            Err(err) => {
                drop(send_child.kill());
                drop(send_child.wait());
                return Err(CommandError::from_spawn_io(err, &receiver_name));
            }
        };

        let recv_status = recv_child.wait()?;
        let send_status = send_child.wait()?;

        if !send_status.success() {
            return Err(CommandError::ExecutionFailed {
                program: sender_name,
                status: send_status,
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        if !recv_status.success() {
            return Err(CommandError::ExecutionFailed {
                program: receiver_name,
                status: recv_status,
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        Ok(())
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
        expect_that!(
            ZpoolCommand::export_pool("backup").to_string(),
            eq("sudo zpool export backup")
        );
    }

    #[googletest::test]
    fn zfs_commands_serialize_expected_cli_strings() {
        expect_that!(
            ZfsCommand::list_dataset("backup/workstation/home").to_string(),
            eq("zfs list -H -o name backup/workstation/home")
        );
        expect_that!(
            ZfsCommand::list_snapshots("rpool/home").to_string(),
            eq("zfs list -H -o name -t snapshot -s creation -d 1 rpool/home")
        );
        expect_that!(
            ZfsCommand::create_container("backup/workstation").to_string(),
            eq("sudo zfs create -p -o mountpoint=none backup/workstation")
        );
        expect_that!(
            ZfsCommand::send_full("rpool/home@snap2").to_string(),
            eq("sudo zfs send -R rpool/home@snap2")
        );
        expect_that!(
            ZfsCommand::send_incremental("@snap1", "rpool/home@snap2").to_string(),
            eq("sudo zfs send -R -I @snap1 rpool/home@snap2")
        );
        expect_that!(
            ZfsCommand::receive("backup/workstation/home").to_string(),
            eq("sudo zfs receive -F -u backup/workstation/home")
        );
        expect_that!(HostnameCommand::new().to_string(), eq("hostname"));
    }

    #[googletest::test]
    fn clapback_runner_executes_stdout_and_piped_commands() -> Result<()> {
        let runner = ClapbackRunner::new();
        let out = runner.stdout_text(&ShCommand {
            script: "printf 'hello-zfs\\n'".to_owned(),
        })?;
        verify_that!(out, eq("hello-zfs"))?;

        let sender = ShCommand {
            script: "printf 'stream-payload'".to_owned(),
        };
        let receiver = ShCommand {
            script: "test \"$(cat)\" = 'stream-payload'".to_owned(),
        };
        verify_that!(runner.run_piped(&sender, &receiver), ok(anything()))
    }
}
