{
  description = "zfs-drive-backup";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    let
      homeManagerModule =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.programs.zfs-drive-backup;
          jsonFormat = pkgs.formats.json { };
          settingsAttr = lib.filterAttrs (_: v: v != null) {
            inherit (cfg.settings) datasets pools;
            machine_name = cfg.settings.machine_name;
          };
        in
        {
          options.programs.zfs-drive-backup = {
            enable = lib.mkEnableOption "zfs-drive-backup, a helper tool for backing up ZFS datasets to external drives";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.zfs-drive-backup;
              defaultText = lib.literalExpression "inputs.zfs-drive-backup.packages.\${pkgs.stdenv.hostPlatform.system}.zfs-drive-backup";
              description = "The zfs-drive-backup package to install.";
            };

            settings = lib.mkOption {
              default = null;
              description = ''
                Configuration written to `$XDG_CONFIG_HOME/zfs-drive-backup/config.json`.
                If `null`, only the binary is installed and no configuration file is managed.
              '';
              type = lib.types.nullOr (
                lib.types.submodule {
                  options = {
                    datasets = lib.mkOption {
                      type = lib.types.nonEmptyListOf lib.types.nonEmptyStr;
                      example = [
                        "rpool/home"
                        "rpool/var"
                      ];
                      description = "ZFS datasets to back up.";
                    };

                    pools = lib.mkOption {
                      type = lib.types.nonEmptyListOf lib.types.nonEmptyStr;
                      example = [
                        "backup-usb"
                        "offsite-pool"
                      ];
                      description = "Candidate external ZFS backup pool names to discover and mount.";
                    };

                    machine_name = lib.mkOption {
                      type = lib.types.nullOr lib.types.nonEmptyStr;
                      default = null;
                      example = "workstation";
                      description = "Optional machine name override for the target dataset prefix on the backup pool.";
                    };
                  };
                }
              );
            };
          };

          config = lib.mkIf cfg.enable (
            lib.mkMerge [
              {
                home.packages = [ cfg.package ];
              }
              (lib.mkIf (cfg.settings != null) {
                xdg.configFile."zfs-drive-backup/config.json".source =
                  jsonFormat.generate "zfs-drive-backup-config.json" settingsAttr;
              })
            ]
          );
        };
    in
    (flake-utils.lib.eachDefaultSystem (
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
          meta = {
            description = "Helper tool for easily backing up ZFS datasets to external hard drives using zfs send";
            mainProgram = "zfs-drive-backup";
          };
        };

        evaluatedHmModule = pkgs.lib.evalModules {
          modules = [
            homeManagerModule
            (
              { lib, ... }:
              {
                options = {
                  home.packages = lib.mkOption {
                    type = lib.types.listOf lib.types.package;
                    default = [ ];
                  };
                  xdg.configFile = lib.mkOption {
                    type = lib.types.attrsOf (
                      lib.types.submodule {
                        options.source = lib.mkOption {
                          type = lib.types.path;
                        };
                      }
                    );
                    default = { };
                  };
                };
                config = {
                  _module.args = { inherit pkgs; };
                  programs.zfs-drive-backup = {
                    enable = true;
                    settings = {
                      datasets = [
                        "rpool/home"
                        "rpool/var"
                      ];
                      pools = [ "backup-usb" ];
                      machine_name = "workstation";
                    };
                  };
                };
              }
            )
          ];
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

        checks = {
          package = zfsDriveBackupPackage;
          homeManagerModule =
            let
              hmCfg = evaluatedHmModule.config;
              generatedJson = builtins.fromJSON (
                builtins.readFile hmCfg.xdg.configFile."zfs-drive-backup/config.json".source
              );
            in
            pkgs.runCommand "check-home-manager-module" { } ''
              ${toString (
                assert builtins.elem zfsDriveBackupPackage hmCfg.home.packages;
                assert generatedJson.datasets == [
                  "rpool/home"
                  "rpool/var"
                ];
                assert generatedJson.pools == [ "backup-usb" ];
                assert generatedJson.machine_name == "workstation";
                ""
              )}
              touch $out
            '';
        };
      }
    ))
    // {
      overlays = {
        default = final: _prev: {
          zfs-drive-backup = self.packages.${final.stdenv.hostPlatform.system}.zfs-drive-backup;
        };
        zfs-drive-backup = self.overlays.default;
      };

      homeManagerModules = {
        default = homeManagerModule;
        zfs-drive-backup = homeManagerModule;
      };
    };
}
