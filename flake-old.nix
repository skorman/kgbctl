{
  description = "KGB CLI tool for research, backtests, and tracking";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
      ];

      forAllSystems = f:
        nixpkgs.lib.genAttrs systems
          (system: f system);

      pkgsFor = system:
        import nixpkgs {
          inherit system;
        };
    in
  {
    devShells = forAllSystems (system:
      let
        pkgs = pkgsFor system;
      in
      {
        default = pkgs.mkShell {
          buildInputs = [
            pkgs.hello
          ];
        };
      }  
    );
  };
}