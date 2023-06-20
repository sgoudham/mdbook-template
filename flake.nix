{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
      craneLib = crane.lib.${system};
      mdbook-template = craneLib.buildPackage {
        src = craneLib.cleanCargoSource (craneLib.path ./.);
      };
    in {
      packages.default = mdbook-template;
      apps.default = flake-utils.lib.mkApp {
        drv = mdbook-template;
      };
      devShells.default = import ./shell.nix {inherit pkgs;};
    });
}
