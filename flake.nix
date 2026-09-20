{
  description = "Graphiga v0 spike development tools";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/6713828a351efa628b025a1adf7f43cbf8597513";
  inputs.fenix.url = "github:nix-community/fenix";
  inputs.fenix.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { nixpkgs, fenix, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = [
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "clippy"
            "rustc"
            "rustfmt"
          ])
        ];
      };
    };
}
