#!/usr/bin/env bash
# Write the Homebrew cask for a released tag, from its SHA256SUMS asset: the .dmg on macOS, the
# AppImage on x86_64 Linux. The tap is mwo-dk/homebrew-bragd (`brew install --cask mwo-dk/bragd/bragd`).
#
#   packaging/taps/generate.sh v1.2.0 <homebrew-tap-dir>
set -euo pipefail
tag="$1" brew="$2"
version="${tag#v}"
repo="https://github.com/mwo-dk/bragd"

# Every sum goes into a variable before a heredoc uses it: set -e does not stop on a failed
# $(...) inside a heredoc, and a missing or empty sum must fail the run, not reach the tap.
sums=$(gh release download "$tag" -R mwo-dk/bragd -p SHA256SUMS -O -)
sum() {
  local s
  s=$(awk -v f="$1" '{ n = $2; sub(/^\*/, "", n) } n == f { print $1 }' <<<"$sums")
  [[ $s =~ ^[0-9a-f]{64}$ ]] || { echo "no sha256 for $1 in SHA256SUMS" >&2; exit 1; }
  echo "$s"
}
d_arm=$(sum "Bragd_${version}_aarch64.dmg")
d_x64=$(sum "Bragd_${version}_x64.dmg")
d_appimage=$(sum "Bragd_${version}_amd64.AppImage")

# Homebrew wants declarative *_steps stanzas: Ruby preflight/postflight blocks are deprecated.
appimage="Bragd_#{version}_amd64.AppImage"
mkdir -p "$brew/Casks"
cat > "$brew/Casks/bragd.rb" <<RUBY
cask "bragd" do
  version "$version"
  sha256 arm:          "$d_arm",
         intel:        "$d_x64",
         x86_64_linux: "$d_appimage"

  on_macos do
    arch arm: "aarch64", intel: "x64"

    url "$repo/releases/download/v#{version}/Bragd_#{version}_#{arch}.dmg"

    app "Bragd.app"
  end
  on_linux do
    url "$repo/releases/download/v#{version}/$appimage"

    depends_on arch: :x86_64

    binary "$appimage", target: "bragd-gui"
  end

  name "Bragd"
  desc "Play, study and keep board games: chess, xiangqi, shogi, go, draughts and more"
  homepage "$repo"

  preflight_steps do
    on_linux do
      set_permissions "Bragd_{{version}}_amd64.AppImage", "0755"
    end
  end

  # Not notarised: without the quarantine flag cleared, macOS says the app is damaged.
  postflight_steps do
    on_macos do
      run "/usr/bin/xattr", args: ["-cr", "{{appdir}}/Bragd.app"]
    end
  end

  # Libraries and settings stay: they are the user's games.
  zap trash: [
    "~/Library/Caches/dk.mwo.bragd",
    "~/Library/WebKit/dk.mwo.bragd",
  ]
end
RUBY
