{
  description = "✨ sleek typing tui with visualized results and historical logging";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }@inputs:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs { inherit system; };
          rustPlatform = pkgs.rustPlatform;
          lib = pkgs.lib;
        in {
          packages.default = rustPlatform.buildRustPackage rec {
            pname = "thokr";
            version = "0.4.1";

            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            # cargoHash = lib.fakeHash;

            meta = {
              description = "✨ sleek typing tui with visualized results and historical logging";
              homepage = "https://github.com/pati08/thokr";
              license = lib.licenses.mit;
              maintainers = [];
            };
          };
        }
      );
}
