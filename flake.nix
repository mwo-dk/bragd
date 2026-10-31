{
  description = "Bragd: play, study and keep board games";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # The package builds its dependencies apart from the workspace (packaging/nix/package.nix).
  inputs.crane.url = "github:ipetkov/crane";

  outputs =
    { self, nixpkgs, crane }:
    let
      # The app is Tauri on WebKitGTK: Linux only. macOS has the .dmg and the Homebrew cask.
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: {
        bragd = pkgs.callPackage ./packaging/nix/package.nix { craneLib = crane.mkLib pkgs; };
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.bragd;
      });

      apps = forAll (pkgs: {
        default = {
          type = "app";
          program = nixpkgs.lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.bragd;
        };
      });
    };
}
