# zfs-drive-backup

[![CI](https://github.com/drmartinwimmer/zfs-drive-backup/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/zfs-drive-backup/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/drmartinwimmer/zfs-drive-backup/blob/main/LICENSE)
[![Built with Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![MSRV: 1.94+](https://img.shields.io/badge/MSRV-1.94%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

Helper tool for easily backing up ZFS datasets to external hard drives using `zfs send`.

## Overview

`zfs-drive-backup` automates backing up local ZFS datasets to external hard drives:

- Mounts the target external backup drive/pool.
- Discovers configured directories and ZFS datasets to back up.
- Replicates snapshots to the external drive using `zfs send` / `zfs receive`.
- Configurable via a simple user configuration file.

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
