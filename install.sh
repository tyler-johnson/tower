#!/bin/sh
# tower installer for linux/macOS:
#   curl -fsSL https://raw.githubusercontent.com/tyler-johnson/tower/main/install.sh | sh
#
# Downloads the latest release binary for this platform, verifies its
# sha256 against the release's checksums.txt, and installs it to
# ~/.local/bin (override with TOWER_INSTALL_DIR; pin a version with
# TOWER_VERSION, e.g. TOWER_VERSION=v0.1.0). Windows: use install.ps1.
set -eu

REPO="tyler-johnson/tower"
INSTALL_DIR="${TOWER_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Linux)  os=linux ;;
  Darwin) os=darwin ;;
  *) echo "tower installer: unsupported OS $(uname -s) — on Windows use install.ps1; elsewhere: cargo install --git https://github.com/$REPO ff-tower-cli" >&2
     exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64)  arch=amd64 ;;
  aarch64|arm64) arch=arm64 ;;
  *) echo "tower installer: unsupported architecture $(uname -m) — try: cargo install --git https://github.com/$REPO ff-tower-cli" >&2
     exit 1 ;;
esac

# fetch <url> <dest>: curl or wget, whichever exists.
if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL -o "$2" "$1"; }
  # The releases/latest page redirects to .../tag/<version> — the header
  # names the version without touching the rate-limited API.
  latest() { curl -fsSI "https://github.com/$REPO/releases/latest" | tr -d '\r' | sed -n 's/^[Ll]ocation:.*\/tag\///p'; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q -O "$2" "$1"; }
  latest() { wget -q -S --max-redirect=0 -O /dev/null "https://github.com/$REPO/releases/latest" 2>&1 | tr -d '\r' | sed -n 's/^ *[Ll]ocation:.*\/tag\///p' | head -n1; }
else
  echo "tower installer: needs curl or wget" >&2
  exit 1
fi

version="${TOWER_VERSION:-$(latest)}"
if [ -z "$version" ]; then
  echo "tower installer: could not determine the latest release — set TOWER_VERSION=vX.Y.Z and re-run" >&2
  exit 1
fi

archive="ff-tower_${version#v}_${os}_${arch}.tar.gz"
base="https://github.com/$REPO/releases/download/$version"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "downloading tower $version ($os/$arch)…"
fetch "$base/$archive" "$tmp/$archive"
fetch "$base/checksums.txt" "$tmp/checksums.txt"

want="$(awk -v f="$archive" '$2 == f {print $1}' "$tmp/checksums.txt")"
if [ -z "$want" ]; then
  echo "tower installer: checksums.txt has no entry for $archive" >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  got="$(sha256sum "$tmp/$archive" | awk '{print $1}')"
else
  got="$(shasum -a 256 "$tmp/$archive" | awk '{print $1}')"
fi
if [ "$got" != "$want" ]; then
  echo "tower installer: checksum mismatch for $archive — refusing to install" >&2
  exit 1
fi

tar -xzf "$tmp/$archive" -C "$tmp" ff-tower
mkdir -p "$INSTALL_DIR"
if command -v install >/dev/null 2>&1; then
  install -m 0755 "$tmp/ff-tower" "$INSTALL_DIR/ff-tower"
else
  cp "$tmp/ff-tower" "$INSTALL_DIR/ff-tower" && chmod 0755 "$INSTALL_DIR/ff-tower"
fi

echo "installed tower $version to $INSTALL_DIR/ff-tower"
case ":$PATH:" in
  *:"$INSTALL_DIR":*) ;;
  *) echo ""
     echo "$INSTALL_DIR is not on your PATH — add it to your shell rc:"
     echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
if ! command -v ff >/dev/null 2>&1; then
  echo ""
  echo "tower is reached through fufu (\`ff tower\`), and no \`ff\` is on your PATH."
  echo "install fufu first:"
  echo "  curl -fsSL https://raw.githubusercontent.com/tyler-johnson/fufu/main/install.sh | sh"
fi
echo ""
echo "next steps:"
echo "  ff tower                       # the board"
