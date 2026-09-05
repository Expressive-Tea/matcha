#!/bin/sh
# Tests install.sh against a local file:// fixture (no network).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
fail() { echo "FAIL: $*" >&2; exit 1; }

# derive this machine's expected triple the same way install.sh does
os=$(uname -s); arch=$(uname -m)
case "$os" in
  Darwin) case "$arch" in arm64|aarch64) triple=aarch64-apple-darwin;; x86_64) triple=x86_64-apple-darwin;; esac;;
  Linux)  case "$arch" in x86_64|amd64) triple=x86_64-unknown-linux-musl;; esac;;
esac
[ -n "${triple:-}" ] || { echo "SKIP: unsupported test host"; exit 0; }

work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
tag=v0.0.0
assetdir="$work/fixture/releases/download/$tag"
mkdir -p "$assetdir"

# build a fake release asset: a tarball containing a 'matcha' executable
printf '#!/bin/sh\necho "matcha 0.0.0-test"\n' > "$work/matcha"
chmod +x "$work/matcha"
tar -czf "$assetdir/matcha-$triple.tar.gz" -C "$work" matcha
if command -v sha256sum >/dev/null 2>&1; then
  ( cd "$assetdir" && sha256sum "matcha-$triple.tar.gz" > "matcha-$triple.tar.gz.sha256" )
else
  ( cd "$assetdir" && shasum -a 256 "matcha-$triple.tar.gz" > "matcha-$triple.tar.gz.sha256" )
fi

# --- case 1: happy path installs the binary ---
bin="$work/bin"
MATCHA_REPO_BASE="file://$work/fixture" MATCHA_VERSION="$tag" MATCHA_INSTALL_DIR="$bin" \
  sh "$here/install.sh" >/dev/null 2>&1 || fail "install.sh exited non-zero on happy path"
[ -x "$bin/matcha" ] || fail "matcha not installed to $bin"
"$bin/matcha" --version >/dev/null 2>&1 || fail "installed matcha not runnable"

# --- case 2: checksum mismatch aborts and does NOT install ---
echo "deadbeef  matcha-$triple.tar.gz" > "$assetdir/matcha-$triple.tar.gz.sha256"
bin2="$work/bin2"
if MATCHA_REPO_BASE="file://$work/fixture" MATCHA_VERSION="$tag" MATCHA_INSTALL_DIR="$bin2" \
     sh "$here/install.sh" >/dev/null 2>&1; then
  fail "install.sh should have failed on checksum mismatch"
fi
[ -e "$bin2/matcha" ] && fail "matcha must NOT be installed on checksum mismatch"

# --- case 3: an unresolvable "latest" explains itself and installs nothing ---
# Both bases point at a path that does not exist, so the redirect and the API
# fallback each come back empty. The branch is worth a test because its failure
# mode is a message: the old one said only "could not resolve latest release
# tag", which is also what a rate-limited API produces.
bin3="$work/bin3"
out=$(MATCHA_REPO_BASE="file://$work/nowhere" MATCHA_API_BASE="file://$work/nowhere" \
        MATCHA_INSTALL_DIR="$bin3" sh "$here/install.sh" 2>&1) && \
  fail "install.sh should have failed with no resolvable latest"
case "$out" in
  *MATCHA_VERSION*) ;;
  *) fail "the unresolvable-latest error must point at MATCHA_VERSION; got: $out" ;;
esac
[ -e "$bin3/matcha" ] && fail "matcha must NOT be installed when latest cannot resolve"

echo "PASS: test_install.sh"
