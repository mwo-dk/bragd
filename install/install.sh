#!/usr/bin/env bash
# Build and install Bragd from source on Linux or macOS.
#
#   ./install/install.sh             build and install
#   ./install/install.sh --yes       answer yes to every question
#
# Installs into $PREFIX (default ~/.local): bin/bragd-gui, bin/bragd-helper and a menu entry on
# Linux. On macOS the app, with the Helper inside, goes to ~/Applications/Bragd.app.
set -euo pipefail

cd "$(dirname "$0")/.."
PREFIX="${PREFIX:-$HOME/.local}"
YES=0
for arg in "$@"; do
  case "$arg" in
    --yes | -y) YES=1 ;;
    -h | --help) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $arg (try --help)" >&2; exit 2 ;;
  esac
done

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
info() { printf '  %s\n' "$*"; }
fail() { printf '\033[31mError:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# Ask politely; Enter means yes.
ask() {
  [ "$YES" = 1 ] && return 0
  local reply
  read -r -p "  $1 [Y/n] " reply </dev/tty || return 1
  [[ -z "$reply" || "$reply" =~ ^[Yy] ]]
}

sudo_cmd() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }

OS="$(uname -s)"
bold "Installing Bragd ($OS)"

# ---------------------------------------------------------------- Rust

if ! have cargo && [ -x "$HOME/.cargo/bin/cargo" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if ! have cargo; then
  bold "Rust is needed to build Bragd, and it isn't installed yet."
  info "The official installer (rustup, https://rustup.rs) puts it in ~/.cargo and ~/.rustup,"
  info "and touches nothing else. You can remove it later with 'rustup self uninstall'."
  if ask "Would you like me to install Rust now?"; then
    have curl || fail "curl is needed to download Rust. Please install curl and run this again."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    fail "No problem. Install Rust from https://rustup.rs when you're ready, then run this again."
  fi
fi
# The newest rust-version among the locked crates for this machine is what Rust must reach
# (1.85, the 2024 edition, when cargo cannot say).
host="$(rustc -vV | sed -n 's/^host: //p')"
need="$(cargo metadata --locked --format-version 1 --filter-platform "$host" 2>/dev/null |
  grep -o '"rust_version":"[0-9.]*"' | cut -d'"' -f4 | sort -V | tail -1 || true)"
need="${need:-1.85}"
have_rust="$(rustc --version | cut -d' ' -f2)"
if [ "$(printf '%s\n%s\n' "$need" "$have_rust" | sort -V | head -1)" != "$need" ]; then
  bold "Your Rust ($(rustc --version)) is older than Bragd needs ($need)."
  if have rustup && ask "Would you like me to update it with 'rustup update stable'?"; then
    rustup update stable
  else
    fail "Please update Rust (rustup update) and run this again."
  fi
fi
info "Rust: $(rustc --version)"

# ---------------------------------------------------------------- system libraries

if [ "$OS" = Linux ] && ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  bold "Bragd needs a few system libraries (WebKitGTK and friends)."
  if have apt-get; then
    PKGS=(apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config)
    PRE=(apt-get update)
  elif have dnf; then
    PKGS=(dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel gcc gcc-c++ make pkgconf-pkg-config)
    PRE=(true)
  elif have pacman; then
    PKGS=(pacman -S --needed --noconfirm webkit2gtk-4.1 base-devel curl wget file openssl libappindicator-gtk3 librsvg pkgconf)
    PRE=(true)
  elif have zypper; then
    PKGS=(zypper install -y webkit2gtk3-devel libopenssl-devel curl wget file libappindicator3-1 librsvg-devel gcc gcc-c++ make pkg-config)
    PRE=(true)
  else
    fail "I don't know this distribution's package manager. See https://v2.tauri.app/start/prerequisites/ and run this again."
  fi
  info "I would run: sudo ${PKGS[*]}"
  if ask "May I install them (this asks for your password)?"; then
    sudo_cmd "${PRE[@]}"
    sudo_cmd "${PKGS[@]}"
  else
    fail "Bragd cannot be built without them. Install them, then run this again."
  fi
fi
if [ "$OS" = Darwin ] && ! xcode-select -p >/dev/null 2>&1; then
  bold "Bragd needs Apple's command line tools."
  if ask "Would you like me to start their installer?"; then
    xcode-select --install || true
  fi
  fail "Finish the Command Line Tools installation, then run this script again."
fi

# ---------------------------------------------------------------- Node.js

if ! have npm; then
  bold "Bragd's interface is built with Node.js, which isn't installed."
  if [ "$OS" = Darwin ] && have brew && ask "Would you like me to install it with Homebrew?"; then
    brew install node
  elif have apt-get && ask "Would you like me to install it with apt (asks for your password)?"; then
    sudo_cmd apt-get install -y nodejs npm
  elif have dnf && ask "Would you like me to install it with dnf (asks for your password)?"; then
    sudo_cmd dnf install -y nodejs npm
  elif have pacman && ask "Would you like me to install it with pacman (asks for your password)?"; then
    sudo_cmd pacman -S --needed --noconfirm nodejs npm
  fi
  have npm || fail "Install Node.js 20.19 or newer from https://nodejs.org, then run this again."
fi
# The build tool (Vite) needs Node.js 20.19+ or 22.12+; a distribution's own package is often older.
node -e 'const [a, b] = process.versions.node.split(".").map(Number); process.exit(a > 22 || (a === 22 && b >= 12) || (a === 20 && b >= 19) ? 0 : 1)' \
  || fail "Node.js $(node -v) is too old to build Bragd. Install 20.19 or newer from https://nodejs.org, then run this again."

# ---------------------------------------------------------------- build

bold "Building Bragd (a few minutes the first time)…"
(cd gui && npm ci --no-audit --no-fund)
# The Helper is a program of its own, which the app starts from beside its own.
cargo build --release --locked -p bragd-helper
if [ "$OS" = Darwin ]; then
  (cd gui && npx tauri build --bundles app)
  mkdir -p "$HOME/Applications" "$PREFIX/bin"
  rm -rf "$HOME/Applications/Bragd.app"
  cp -R target/release/bundle/macos/Bragd.app "$HOME/Applications/"
  install -m 755 target/release/bragd-helper "$HOME/Applications/Bragd.app/Contents/MacOS/bragd-helper"
  ln -sf "$HOME/Applications/Bragd.app/Contents/MacOS/bragd-gui" "$PREFIX/bin/bragd-gui"
  ln -sf "$HOME/Applications/Bragd.app/Contents/MacOS/bragd-helper" "$PREFIX/bin/bragd-helper"
  info "Installed ~/Applications/Bragd.app, with bragd-helper inside"
else
  (cd gui && npx tauri build --no-bundle)
  mkdir -p "$PREFIX/bin"
  install -m 755 target/release/bragd-gui "$PREFIX/bin/bragd-gui"
  install -m 755 target/release/bragd-helper "$PREFIX/bin/bragd-helper"
  ICONS="$PREFIX/share/icons/hicolor/128x128/apps"
  APPS="$PREFIX/share/applications"
  mkdir -p "$ICONS" "$APPS"
  install -m 644 assets/brand/icons/128x128.png "$ICONS/bragd.png"
  cat >"$APPS/bragd.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Bragd
GenericName=Board games
Comment=Play, study and keep board games
Exec="$PREFIX/bin/bragd-gui" %F
Icon=bragd
Terminal=false
Categories=Game;BoardGame;
EOF
  info "Installed $PREFIX/bin/bragd-gui, $PREFIX/bin/bragd-helper and a menu entry"
fi

echo
bold "Done."
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) info "Add $PREFIX/bin to your PATH to run 'bragd-gui' from anywhere, e.g.:"
     info "  echo 'export PATH=\"$PREFIX/bin:\$PATH\"' >> ~/.bashrc" ;;
esac
info "Start Bragd from your app menu, or run: bragd-gui"
