# zfs-drive-backup

[![CI](https://github.com/drmartinwimmer/zfs-drive-backup/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/zfs-drive-backup/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/drmartinwimmer/zfs-drive-backup/blob/main/LICENSE)
[![Built with Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![MSRV: 1.94+](https://img.shields.io/badge/MSRV-1.94%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

Helper tool for easily backing up ZFS datasets to external hard drives using `zfs send`.

## Overview

`zfs-drive-backup` automates backing up local ZFS datasets to external hard drives:

- Reads `~/.config/zfs-drive-backup/config.json` (via `directories`) to determine which ZFS datasets and external backup pools to use.
- Discovers and mounts the target external backup pool via `zpool list` / `sudo zpool import -l <pool>` (built with [`clapback`](https://crates.io/crates/clapback)).
- Replicates snapshots to `<backup_pool>/<machine_name>/<dataset_path>` using `sudo zfs send -R [-I @<common_snap>] <source>@<latest_snap> | sudo zfs receive -F -u <target>`.
- Optionally exports the backup pool at the end when `--export` is specified (`sudo zpool export <pool>`).

## Configuration

Create `~/.config/zfs-drive-backup/config.json`:

```json
{
  "datasets": ["tuxedo/home", "tuxedo/var"],
  "pools": ["backup-usb"],
  "machine_name": "tuxedo"
}
```

- `datasets`: List of local ZFS datasets to back up. The source pool prefix is stripped and mapped under `<backup_pool>/<machine_name>/<dataset_path>` (e.g. `tuxedo/home` -> `backup-usb/tuxedo/home`).
- `pools`: Candidate external ZFS backup pool names to check or import.
- `machine_name` (optional): Override for the machine dataset prefix; defaults to the system `hostname`.

## Usage

```bash
# Run backup using ~/.config/zfs-drive-backup/config.json
zfs-drive-backup

# Run backup and export the external ZFS pool when finished
zfs-drive-backup --export

# Use a custom configuration file path
zfs-drive-backup --config /path/to/config.json --export
```

## Development

This repository uses [Nix](https://nixos.org/) flakes, [Direnv](https://direnv.net/), and [Jujutsu (`jj`)](https://github.com/martinvonz/jj):

```bash
# Allow direnv to load development environment
direnv allow

# Check code formatting & lints
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
purist

# Run tests
cargo test --all-targets --all-features

# Build with Nix
nix build
```

## License

MIT
