# The app, `bragd-gui` (Tauri and WebKitGTK), built from this checkout. Linux only.
# The frontend is built first; `cargo build` then embeds gui/dist, as in a local build.
# Built with crane in two steps: the dependencies alone, then the workspace, so a change to our
# own crates does not compile every dependency again.
{
  lib,
  craneLib,
  importNpmLock,
  nodejs,
  pkg-config,
  wrapGAppsHook3,
  makeDesktopItem,
  copyDesktopItems,
  openssl,
  webkitgtk_4_1,
  librsvg,
  glib-networking,
}:

let
  common = {
    pname = "bragd";
    version = (lib.importTOML ../../Cargo.toml).workspace.package.version;
    src = lib.cleanSource ../..;
    strictDeps = true;
    cargoExtraArgs = "--locked -p bragd-gui";
    # The workspace's tests run in CI (ci.yml), on every system.
    doCheck = false;
    # wrapGAppsHook3 in both steps: it adds GTK's paths to PKG_CONFIG_PATH, and a change there
    # would run the -sys crates' build scripts again, and all built on them.
    nativeBuildInputs = [ pkg-config wrapGAppsHook3 ];
    buildInputs = [ openssl webkitgtk_4_1 librsvg glib-networking ];
    # Built from our tags: the update check names `nix` as the way to upgrade.
    env.BRAGD_FROM_FLAKE = "1";
  };
  cargoArtifacts = craneLib.buildDepsOnly (
    common
    // {
      cargoCheckCommand = "true";
    }
  );
in
craneLib.buildPackage (
  common
  // {
    inherit cargoArtifacts;

    # npm's packages from gui/package-lock.json, without a hash to keep up to date.
    npmRoot = "gui";
    npmDeps = importNpmLock { npmRoot = ../../gui; };

    nativeBuildInputs = common.nativeBuildInputs ++ [
      nodejs
      importNpmLock.npmConfigHook
      copyDesktopItems
    ];

    preBuild = ''
      (cd gui && npm run build)
    '';

    desktopItems = [
      (makeDesktopItem {
        name = "bragd";
        desktopName = "Bragd";
        genericName = "Board games";
        comment = "Play, study and keep board games";
        exec = "bragd-gui %F";
        icon = "bragd";
        categories = [ "Game" "BoardGame" ];
      })
    ];

    postInstall = ''
      install -Dm644 gui/src-tauri/icons/128x128.png $out/share/icons/hicolor/128x128/apps/bragd.png
    '';

    passthru = { inherit cargoArtifacts; };

    meta = {
      description = "Play, study and keep board games: chess, xiangqi, shogi, go, draughts and more";
      homepage = "https://github.com/mwo-dk/bragd";
      changelog = "https://github.com/mwo-dk/bragd/blob/master/docs/changelog.md";
      license = lib.licenses.mit;
      mainProgram = "bragd-gui";
      platforms = lib.platforms.linux;
    };
  }
)
