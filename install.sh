#!/bin/sh
# matcha installer — downloads a prebuilt binary from GitHub Releases.
#   curl -fsSL <raw>/install.sh | sh
# Env: MATCHA_VERSION (tag, default latest), MATCHA_INSTALL_DIR (default ~/.local/bin),
#      MATCHA_REPO_BASE / MATCHA_API_BASE (override to install from the internal Gitea,
#      whose release API is GitHub-compatible for everything this script touches).
set -eu

# Public by default: the Gitea host is unreachable outside the network, and this script
# is what crates.io and the README point people at.
REPO_BASE="${MATCHA_REPO_BASE:-https://github.com/Expressive-Tea/matcha}"
# The API lives on its own host, NOT under the repo path. Keep it separate from
# REPO_BASE (which is used for release DOWNLOAD urls under the repo).
API_BASE="${MATCHA_API_BASE:-https://api.github.com/repos/Expressive-Tea/matcha}"
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
      arm64|aarch64) triple="aarch64-unknown-linux-musl" ;;
      *) err "unsupported Linux arch: $arch" ;;
    esac ;;
  *) err "unsupported OS: $os — try: cargo install --git $REPO_BASE" ;;
esac

# Resolving "latest" prefers the /releases/latest redirect over the release API.
# The API is rate-limited to 60 requests an hour per IP unauthenticated, which an
# office behind one NAT or a CI runner burns through — and when it trips, the
# response carries no tag_name, so the old code failed with "could not resolve
# latest release tag" and no hint of why. The redirect has no such limit and
# Gitea implements it the same way. The API stays as the fallback, since that is
# the path wget can follow without parsing headers.
resolve_latest() {
  if command -v curl >/dev/null 2>&1; then
    effective=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$REPO_BASE/releases/latest" 2>/dev/null || true)
    case "$effective" in
      */releases/tag/*) echo "${effective##*/}"; return 0 ;;
    esac
  fi
  fetch "$API_BASE/releases/latest" 2>/dev/null \
    | grep -o '"tag_name":[[:space:]]*"[^"]*"' | head -1 | cut -d'"' -f4
}

version="${MATCHA_VERSION:-latest}"
if [ "$version" = "latest" ]; then
  tag=$(resolve_latest)
  [ -n "$tag" ] || err "could not resolve the latest release tag.
  The release API may be rate-limited (60/hour per IP without a token).
  Pin a version instead:  curl -fsSL <raw>/install.sh | MATCHA_VERSION=v26.8.0-beta.0 sh"
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
