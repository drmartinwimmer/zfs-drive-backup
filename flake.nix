{
  description = "zfs-drive-backup";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
        zfsDriveBackupPackage = rustPlatform.buildRustPackage {
          pname = "zfs-drive-backup";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
      in
      {
        # Development shell
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              toolchain
              (cargo-audit.override { inherit rustPlatform; })
              taplo
              prettier
            ];
          };

        packages = {
          default = zfsDriveBackupPackage;
          zfs-drive-backup = zfsDriveBackupPackage;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = zfsDriveBackupPackage;
          };
          zfs-drive-backup = flake-utils.lib.mkApp {
            drv = zfsDriveBackupPackage;
          };
        };
      }
    );
}
