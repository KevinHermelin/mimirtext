{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      naersk,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        naersk' = pkgs.callPackage naersk { };

        fontsConf = pkgs.makeFontsConf {
          fontDirectories = [
            pkgs.jetbrains-mono
          ];
        };
      in
      rec {
        packages = {
          default = naersk'.buildPackage {
            src = ./.;
          };

          test = naersk'.buildPackage {
            src = ./.;
            mode = "test";
          };

          demo = pkgs.stdenv.mkDerivation {
            name = "mimir-demo";

            src = ./demo;

            nativeBuildInputs = [
              packages.default
              pkgs.vhs
              pkgs.ncurses
            ];

            buildPhase = ''
              # vhs seems to require a home.
              export HOME="$TMPDIR/home"
              export FONTCONFIG_FILE="${fontsConf}"

              mkdir -p "$HOME"

              vhs ${./demo/demo.tape}
              mkdir -p $out
              cp *.gif $out/
            '';
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rustfmt
            pkgs.cargo-insta
            pkgs.clippy
          ];
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };
      }
    );
}
