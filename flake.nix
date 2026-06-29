{
  description = "KGB CLI tool for research, backtests, and tracking";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs, ... }@inputs:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-linux"
      ];

      pkgsFor = system: import nixpkgs { inherit system; };
      forAllSystems = func: nixpkgs.lib.genAttrs systems (system: func system);
    in
  {
    devShells = forAllSystems (system:
      let
        pkgs = pkgsFor system;
      in
      {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.clippy
          ];
        };
      }
    );

    packages = forAllSystems (system:
      let
        pkgs = pkgsFor system;
      in
      {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "kgbctl";

          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
      }
    );
  };
}
      # let
      #   pkgs = system: inputs.nixpkgs.lib.genAttrs [ "hello" ] (package :
      #     (pkgsFor "aarch64-darwin").${package}
      #   );
      # in
      # inputs.nixpkgs.lib.genAttrs systems pkgs;


    # packages."aarch64-darwin" = inputs.nixpkgs.lib.genAttrs [ "hello" ] (package :
    #   (pkgsFor "aarch64-darwin").${package}
    # );

    # packages."aarch64-darwin".hello = (pkgsFor "aarch64-darwin").hello;
    # devShells."aarch64-darwin".hello = (pkgsFor "aarch64-darwin").hello;
    # packages."aarch64-darwin".default
#   };
# }