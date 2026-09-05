#!/usr/bin/env bash
#
# Scaffolds a starter with the matcha binary, installs it, type-checks it, boots
# it, and asks it for its graph.
#
# This is the check the repository did not have. `cargo test` proves the CLI
# writes the files it means to write; it cannot prove those files describe a
# project the current `@green-tea/core` will run — which is exactly how the
# templates sat two core releases behind for two months without a red build.
#
# Usage: scripts/starter-smoke.sh <node|deno|bun>
set -euo pipefail

RUNTIME="${1:?usage: starter-smoke.sh <node|deno|bun>}"
MATCHA="${MATCHA:-$PWD/target/debug/matcha}"
[ -x "$MATCHA" ] || { echo "no matcha binary at $MATCHA (cargo build first)" >&2; exit 2; }

case "$RUNTIME" in
  node) TEMPLATE_PORT=3000; INSTALL=(npm install); CHECK=(npx --yes tsc --noEmit) ;;
  deno) TEMPLATE_PORT=8000; INSTALL=(deno install); CHECK=(deno check src/main.ts) ;;
  bun)  TEMPLATE_PORT=8000; INSTALL=(bun install);  CHECK=(bunx --bun tsc --noEmit) ;;
  *) echo "unknown runtime: $RUNTIME" >&2; exit 2 ;;
esac

# The starter hardcodes its port, so the script rewrites it to a free one rather
# than colliding with whatever the person running this already has on 3000. A
# check that only survives on a clean CI runner is a check nobody runs locally.
PORT="${PORT:-$((20000 + RANDOM % 20000))}"

WORK="$(mktemp -d)"
SERVER=""
cleanup() {
  [ -n "$SERVER" ] && { pkill -P "$SERVER" 2>/dev/null || true; kill "$SERVER" 2>/dev/null || true; }
  rm -rf "$WORK"
}
trap cleanup EXIT

cd "$WORK"
echo "▸ matcha new demo --runtime $RUNTIME"
"$MATCHA" new demo --runtime "$RUNTIME"
cd demo
sed -i.bak "s/$TEMPLATE_PORT/$PORT/g" src/main.ts && rm -f src/main.ts.bak

echo "▸ ${INSTALL[*]}"
"${INSTALL[@]}"

echo "▸ ${CHECK[*]}"
"${CHECK[@]}"

echo "▸ matcha run (boot on :$PORT)"
"$MATCHA" run >"$WORK/server.log" 2>&1 &
SERVER=$!
for _ in $(seq 1 60); do
  curl -fsS -o /dev/null "http://localhost:$PORT/" 2>/dev/null && break
  sleep 1
done

# The starter's two halves: the @Html page, and the @Sse stream that is the
# reason the starter is alive rather than a JSON hello.
code=$(curl -fsS -o /dev/null -w '%{http_code}' "http://localhost:$PORT/" 2>/dev/null || echo 000)
[ "$code" = 200 ] || { echo "GET / returned $code"; echo '--- server log ---'; cat "$WORK/server.log"; exit 1; }
# Read one frame and stop. `head -1` closing the pipe kills curl with SIGPIPE,
# which under `pipefail` fails the pipeline for a stream that arrived exactly as
# intended — hence the capture-then-compare instead of a piped `grep -q`.
zen="$(curl -fsS -N --max-time 5 "http://localhost:$PORT/zen" 2>/dev/null | head -1 || true)"
case "$zen" in
  'data: {"zen":'*) ;;
  *) echo "GET /zen did not stream an SSE frame (got: ${zen:-<nothing>})"; cat "$WORK/server.log"; exit 1 ;;
esac
echo "✓ serves / and streams /zen"

pkill -P "$SERVER" 2>/dev/null || true; kill "$SERVER" 2>/dev/null || true; SERVER=""

# Introspection runs against the same project, so it also proves the entry file
# the templates scaffold still exports the `app` that graph/explain import.
echo "▸ matcha graph"
"$MATCHA" graph | grep -q '^flowchart LR' || { echo "graph produced no mermaid"; exit 1; }
"$MATCHA" graph --format dot | grep -q '^digraph green_tea' || { echo "graph produced no DOT"; exit 1; }
"$MATCHA" explain /zen | grep -q '\[sse\]' || { echo "explain did not report the sse route"; exit 1; }
echo "✓ graph and explain resolve"

# Every `matcha add` capability, spliced into one controller and type-checked
# together. The stubs name core's real surface — `sse()` with an id, `@Ws` with
# `@inbound()`, `MultipartBody`, `@Transformer` — so this is the thing that
# notices when that surface moves. No peer dependency is installed on purpose:
# core lazy-requires `ws` and `busboy`, so neither is needed to compile, and a
# check that needed them would be hiding that fact.
echo "▸ matcha add (every capability)"
for cap in sse ws upload stream buffer; do
  "$MATCHA" add "$cap" >/dev/null || { echo "matcha add $cap failed"; exit 1; }
done
"${CHECK[@]}" || { echo "the generated handlers do not type-check"; exit 1; }
echo "✓ every add capability type-checks"

echo "✓ $RUNTIME starter is good"
