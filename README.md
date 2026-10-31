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
  "datasets": ["rpool/home", "rpool/var"],
  "pools": ["backup-usb"],
  "machine_name": "workstation"
}
```

- `datasets`: List of local ZFS datasets to back up. The source pool prefix is stripped and mapped under `<backup_pool>/<machine_name>/<dataset_path>` (e.g. `rpool/home` -> `backup-usb/workstation/home`).
- `pools`: Candidate external ZFS backup pool names to check or import.
- `machine_name` (optional): Override for the machine dataset prefix; defaults to the system `hostname`.

## Nix & Home Manager Installation

Add `zfs-drive-backup` to your flake `inputs`:

```nix
{
  inputs = {
    zfs-drive-backup = {
      url = "github:drmartinwimmer/zfs-drive-backup";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
```

### Home Manager Module

Import `inputs.zfs-drive-backup.homeManagerModules.default` and configure `programs.zfs-drive-backup`:

```nix
{ inputs, ... }:
{
  imports = [
    inputs.zfs-drive-backup.homeManagerModules.default
  ];

  programs.zfs-drive-backup = {
    enable = true;
    settings = {
      datasets = [
        "rpool/home"
        "rpool/var"
      ];
      pools = [ "backup-usb" ];
    };
  };
}
```

### Nixpkgs Overlay

You can also expose `pkgs.zfs-drive-backup` via the flake overlay:

```nix
{ inputs, pkgs, ... }:
{
  nixpkgs.overlays = [ inputs.zfs-drive-backup.overlays.default ];
  environment.systemPackages = [ pkgs.zfs-drive-backup ];
}
```

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
