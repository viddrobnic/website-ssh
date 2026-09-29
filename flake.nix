{
  description = "Service for accessing viddrobnic.com via SSH";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{
      self,
      crane,
      flake-parts,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];

      perSystem =
        { config, pkgs, ... }:
        let
          craneLib = crane.mkLib pkgs;

          src = craneLib.cleanCargoSource ./.;

          commonArgs = {
            inherit src;
            strictDeps = true;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          website-ssh = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );
        in
        {
          checks = {
            inherit website-ssh;

            website-ssh-clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-features";
              }
            );

            # Check formatting
            website-ssh-fmt = craneLib.cargoFmt {
              inherit src;
            };

            website-ssh-toml-fmt = craneLib.taploFmt {
              src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
            };
          };

          packages.default = website-ssh;

          apps.default = {
            type = "app";
            program = "${website-ssh}/bin/website-ssh";
            meta.description = "Run website-ssh.";
          };

          devShells.default = craneLib.devShell {
            checks = config.checks;
          };

          formatter = pkgs.nixfmt-tree;
        };

      flake.nixosModules.default =
        {
          lib,
          pkgs,
          config,
          ...
        }:
        let
          cfg = config.services.website-ssh;
        in
        {
          options.services.website-ssh = {
            enable = lib.mkEnableOption "website-ssh";

            port = lib.mkOption {
              type = lib.types.port;
              default = 2222;
            };
          };

          config = lib.mkIf cfg.enable {
            systemd.services.website-ssh = {
              description = "Website SSH";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];

              serviceConfig = {
                ExecStart = "${
                  self.packages.${pkgs.stdenv.hostPlatform.system}.default
                }/bin/website-ssh -p ${toString cfg.port}";

                DynamicUser = true;
                StateDirectory = "website-ssh";
                WorkingDirectory = "/var/lib/website-ssh";

                Restart = "on-failure";
                RestartSec = 5;

                StandardOutput = "journal";
                StandardError = "journal";

                AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
              };
            };
          };
        };
    };
}
