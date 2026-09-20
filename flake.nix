{
  description = "Graphiga v0 spike development tools";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/6713828a351efa628b025a1adf7f43cbf8597513";

  outputs = { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc rustfmt clippy ];
      };
    };
}
