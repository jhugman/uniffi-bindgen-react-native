{
  description = "Development environment for uniffi-bindgen-react-native";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-darwin.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    # Hermes rn/0.77-stable uses a CMake policy removed in CMake 4.
    nixpkgs-cmake3.url = "github:NixOS/nixpkgs/nixos-24.11";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      nixpkgs-darwin,
      nixpkgs-cmake3,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import (if system == "x86_64-darwin" then nixpkgs-darwin else nixpkgs) {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          cmakePkgs = import nixpkgs-cmake3 { inherit system; };
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "clippy"
              "rustfmt"
            ];
            targets = [ "wasm32-unknown-unknown" ];
          };
          wasmBindgenCli = pkgs.rustPlatform.buildRustPackage rec {
            pname = "wasm-bindgen-cli";
            version = "0.2.114";

            src = pkgs.fetchCrate {
              inherit pname version;
              hash = "sha256-xrCym+rFY6EUQFWyWl6OPA+LtftpUAE5pIaElAIVqW0=";
            };
            cargoHash = "sha256-Z8+dUXPQq7S+Q7DWNr2Y9d8GMuEdSnq00quUR0wDNPM=";
          };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              cargo-ndk
              cargo-nextest
              wasmBindgenCli
              nodejs_24
              python3
              yarn
              cmakePkgs.cmake
              cmakePkgs.mdbook
              ninja
              clang-tools
              pkg-config
            ];

            shellHook = ''
              export CARGO_TERM_COLOR=always
            '';
          };
        }
      );
    };
}
