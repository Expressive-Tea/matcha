#!/bin/sh
# matcha installer — downloads a prebuilt binary from Gitea Releases.
#   curl -fsSL <raw>/install.sh | sh
# Env: MATCHA_VERSION (tag, default latest), MATCHA_INSTALL_DIR (default ~/.local/bin),
#      MATCHA_REPO_BASE (default the Gitea repo URL).
set -eu

REPO_BASE="${MATCHA_REPO_BASE:-https://git.svc.zoit.services/Green-Tea/matcha}"
INSTALL_DIR="${MATCHA_INSTALL_DIR:-$HOME/.local/bin}"

err() { echo "matcha-install: $*" >&2; exit 1; }

if command -v curl >/dev/null 2>&1; then
  fetch_out() { curl -fsSL "$1" -o "$2"; }
  fetch()     { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch_out() { wget -qO "$2" "$1"; }
  fetch()     { wget -qO- "$1"; }
else
  err "need curl or wget"
fi

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | awk '{print $1}'; }
else
  err "need sha256sum or shasum"
fi

os=$(uname -s); arch=$(uname -m)
case "$os" in
  Darwin)
    case "$arch" in
      arm64|aarch64) triple="aarch64-apple-darwin" ;;
      x86_64)        triple="x86_64-apple-darwin" ;;
      *) err "unsupported macOS arch: $arch" ;;
    esac ;;
  Linux)
    case "$arch" in
      x86_64|amd64)  triple="x86_64-unknown-linux-musl" ;;
      arm64|aarch64) err "linux arm64 has no prebuilt binary yet — use: cargo install --git $REPO_BASE" ;;
      *) err "unsupported Linux arch: $arch" ;;
    esac ;;
  *) err "unsupported OS: $os — try: cargo install --git $REPO_BASE" ;;
esac

version="${MATCHA_VERSION:-latest}"
if [ "$version" = "latest" ]; then
  tag=$(fetch "$REPO_BASE/api/v1/repos/Green-Tea/matcha/releases/latest" \
        | grep -o '"tag_name":"[^"]*"' | head -1 | cut -d'"' -f4)
  [ -n "$tag" ] || err "could not resolve latest release tag"
else
  tag="$version"
fi

asset="matcha-${triple}.tar.gz"
url="$REPO_BASE/releases/download/${tag}/${asset}"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
echo "matcha-install: downloading $asset ($tag)"
fetch_out "$url" "$tmp/$asset"          || err "download failed: $url"
fetch_out "$url.sha256" "$tmp/$asset.sha256" || err "checksum download failed: $url.sha256"

want=$(awk '{print $1}' < "$tmp/$asset.sha256")
got=$(sha256 "$tmp/$asset")
[ -n "$want" ] || err "empty checksum file"
[ "$want" = "$got" ] || err "checksum mismatch (want $want, got $got)"

tar -xzf "$tmp/$asset" -C "$tmp"
[ -f "$tmp/matcha" ] || err "archive did not contain 'matcha'"
mkdir -p "$INSTALL_DIR"
chmod +x "$tmp/matcha"
mv "$tmp/matcha" "$INSTALL_DIR/matcha"

echo "matcha-install: installed to $INSTALL_DIR/matcha ($tag)"
case ":${PATH}:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "matcha-install: add to PATH ->  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
"$INSTALL_DIR/matcha" --version 2>/dev/null || true
