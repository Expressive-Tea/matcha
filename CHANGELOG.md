# Changelog

All notable changes to `matcha-cli` are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

matcha uses calendar versioning: `YY.M.PATCH` — the month is not zero-padded, matching
green-tea's convention. **The CLI ships on its own version line**: `matcha` 26.8 does not
mean `@green-tea/core` 26.8, and the two release independently. The core version a scaffold
is pinned to is `CORE_VERSION` in `src/template.rs`, and each entry below says when it moved.

## [Unreleased]

### Added

- **`matcha graph` and `matcha explain <pattern>`.** green-tea derives execution order from
  what each node declares it `needs` and `provides`, which makes the graph a real artifact
  rather than a diagram someone drew — and the CLI could not draw any of it, though
  `toMermaid`, `toDOT`, `graph` and `explain` had been exported all along. `graph` prints
  Mermaid, Graphviz DOT or the raw `GraphView` (`--format`, `--out`); `explain` prints one
  route's chain in execution order with each node's needs and provides.

  Both import the app and call `app.ready()`, which resolves the graph and is documented as
  deliberately *not* running provider factories — asking for a diagram must not open your
  database connections. `explain` takes the route pattern as declared (`/users/:id`, not
  `/users/42`), and a miss lists the patterns that are registered.

- **`matcha doctor`** — the misconfigurations that fail confusingly rather than loudly:
  `experimentalDecorators` missing, which turns every `@Route` into a syntax error that
  blames the decorator; a core pin that cannot resolve, or that does not match what is
  installed; an entry that does not export `app`; and `@Ws` or multipart routes whose
  lazy-loaded peer dependency is absent, which compile and then fail at the first request.
  Every check is a file read — no runtime spawned, nothing fetched — so it works before the
  project installs. Exits non-zero on a failure.

- **`matcha new --runtime edge`** — a Cloudflare Worker: `wrangler.toml` with
  `nodejs_compat`, `edgeHandler(app)` as the default export, and wrangler's `[assets]`
  serving `./public`, so `/` is the same page as on the other three runtimes and `/zen` the
  same stream from the worker. `matcha run` is `wrangler dev`, which boots real workerd, so
  edge stops being a deploy-only target. Detection puts `wrangler.toml` ahead of
  `package.json`, since an edge project has both.

  Core is installed from **JSR** on this runtime alone. The npm ESM build carries tsup's
  `createRequire(import.meta.url)` banner at module scope and `import.meta.url` is undefined
  on workerd, so `@green-tea/core/edge` from npm throws before any of its own code runs
  ([green-tea#93](https://github.com/Expressive-Tea/green-tea/issues/93)). The scaffolded
  `.npmrc` says so; it and the alias can both go once that is fixed.

- **`matcha add ws` and `matcha add upload`**, and **`--controller <path>`**. `add` used to
  refuse outright when a project had more than one controller; it still declines to guess,
  but it lists the candidates and the flag picks. Both new capabilities name the optional
  peer dependency they need on success — core lazy-requires `ws` and `busboy`, so a missing
  one is an error at the first request rather than at boot, and the moment to mention it is
  when the handler is written.

- **`scripts/starter-smoke.sh`** — scaffolds, installs, type-checks, boots, curls `/` and the
  `/zen` stream, introspects, and adds every capability, per runtime. CI runs it on a matrix
  of all four, and a weekly job fails when the scaffold's core pin falls behind the `beta`
  dist-tag.

### Changed

- **The scaffold composes its graph in `src/app.ts`.** `src/main.ts` now holds only the
  runtime's serve call and imports the app from it. The split is what lets `graph`, `explain`
  and `doctor` read an application without binding a port. `create module` follows, preferring
  `src/app.ts` and falling back to `src/main.ts` so projects scaffolded before the split keep
  working.

- **`add sse` generates a resumable stream.** The stub predates green-tea 26.9: it yielded
  bare values, so an `EventSource` reconnect restarted the iterable from its beginning and
  lost the gap in silence. It now tags each event with `sse(data, { id })` and reads
  `@header('last-event-id')` to resume from it.

- **`add buffer` demonstrates `@Transformer`.** It used to return `Buffer.from('hello')`,
  which is not what the capability's name means — "buffer" is the *transport*, the whole
  response at once — and never worked: `TransformerFn` returns `body: string` and the default
  `JsonTransformer` is `JSON.stringify`, so the route answered
  `{"type":"Buffer","data":[104,...]}`. It now shapes a `text/csv` response through the
  extension point that actually governs one.

- **Types in a decorated signature are imported with `import type`.** `wire`'s import
  handling became type-aware in both directions: merging a value symbol into an
  `import type` line would erase the decorator at runtime, and merging a type into a value
  line is TS1272 under `isolatedModules` with `emitDecoratorMetadata` — which node's and
  bun's `tsc` accept and Deno rejects, so the same generated handler compiled on two runtimes
  and failed on the third.

### Fixed

- **The scaffold no longer pins `@green-tea/core` two releases behind.** Every runtime
  overlay carried `^26.7.0-beta.0`. Core is prerelease-only, and npm's semver excludes a
  prerelease from any range whose comparators carry a different `major.minor.patch` — so that
  caret was not a loose pin but a pin to the oldest matching prerelease that nothing could
  move off. Every project `matcha new` produced since July installed 26.7.0-beta.0 and could
  not upgrade to it: no logger, no request budget, no resumable `@Sse` id, and none of the
  crashes 26.9 fixed. The pin is exact now, lives in one `CORE_VERSION` constant, and the
  scaffold is on **26.9.0-beta.1**.

- **The Deno scaffold resolves core through JSR** rather than `npm:`. JSR is core's native
  path for Deno and the one green-tea 26.9 repaired; the npm build's `createRequire` banner
  never existed there, which is what used to break `@Html` and multipart at boot.

- **The bun scaffold type-checks.** Its `tsconfig.json` declared no Bun types, so `Bun.serve`
  was `TS2868: Cannot find name 'Bun'` — meaning `tsc --noEmit`, and therefore
  `matcha create --check`, failed on every bun project the CLI has ever scaffolded.

- **A prebuilt binary for Linux arm64.** The installer used to refuse that platform and
  send you to `cargo install --git` — so Graviton, a Raspberry Pi, an ARM CI runner and
  Docker on Apple Silicon all needed a Rust toolchain. `rustup target add` alone could
  never have produced it: `tree-sitter` and `tree-sitter-typescript` compile C through the
  `cc` crate, and Ubuntu ships no aarch64 musl cross compiler. Both Linux targets now build
  through zig, which is one, so the arch that was missing and the arch that worked share a
  toolchain instead of having two.

  Each release job also runs the binary it just built, wherever the runner can execute it —
  a binary that links and will not start is the failure a release pipeline must not leave
  for a user's terminal to find. Both macOS binaries run on the arm64 macOS runner, the
  Intel one through Rosetta, which is probed rather than assumed since a runner without it
  would otherwise fail a release over its own configuration; and each Linux binary runs wherever
  `ubuntu-latest` resolves to its architecture. Because that last part depends on the
  runner pool rather than on the pipeline, a separate job pins an arm64 Linux runner and
  installs the published binary there through `install.sh` — so the arm64 artifact is
  started before it reaches anyone, on every release, and the installer's arm64 branch and
  checksum verification are exercised against the real assets on the way.

- **`install.sh` resolves `latest` without the release API.** The API is rate-limited to
  60 requests an hour per IP unauthenticated — an office behind one NAT or a CI runner
  burns through it — and a throttled response carries no `tag_name`, so the installer
  failed with "could not resolve latest release tag" and no hint of the cause. It now
  follows the `/releases/latest` redirect, which has no such limit and which Gitea
  implements the same way, keeping the API as the fallback for the wget path. The error
  that remains names the rate limit and points at `MATCHA_VERSION`.

- **A beta release is flagged as a prerelease.** Nothing set the flag, so every beta
  answered `/releases/latest` — which is what `install.sh` resolves when no
  `MATCHA_VERSION` is given. Harmless while every release is a beta, and wrong the moment
  a stable one exists: the next beta published after it would displace it for everyone
  installing with the default.

- **CI runs `test_install.sh`.** It has existed since the installer landed and no workflow
  ever ran it. It now runs on Linux and macOS, with `shellcheck` alongside, and covers a
  third case: an unresolvable `latest` must explain itself and install nothing.

- **`MIT` is a file, not just a manifest field.** `Cargo.toml` declared `license = "MIT"` and
  the published crate carried no license text. Added along with the `DCO` the README has been
  requiring sign-off against, and a `CONTRIBUTING.md` saying how.

## [26.8.0-beta.0] - 2026-08-07

### Added

- Publishing to crates.io via OIDC Trusted Publishing — no token in CI.

### Changed

- The crate is `matcha-cli` on crates.io (the name `matcha` was taken); the installed binary
  stays `matcha`, so `cargo install matcha-cli` still gives you `matcha`. Two-forge CI/CD:
  Gitea is origin, GitHub is the public mirror that release tags are promoted to.

### Fixed

- The README's install paths point at hosts that actually resolve.

## [26.7.0] - 2026-07-13

First tagged release. A single native binary, no JS runtime needed to install it.

### Added

- **`matcha new`** — scaffolds from an embedded starter that is alive on first boot: it serves
  an `index.html` and streams a rotating zen message over `@Sse('/zen')`. `--runtime` picks
  node (the default), deno or bun through a shared template plus a per-runtime overlay;
  `--template-url` scaffolds from any git template, with a `gh:owner/repo` shorthand.
- **`matcha run`** — detects the runtime and spawns it in watch mode. Precedence, first match
  wins: `matcha.toml`, then `deno.json`, then a bun lockfile, then `package.json`.
- **`matcha create <controller|step|provider|module>`** — generates the piece and wires it in
  with tree-sitter: imports merge into an existing brace rather than adding a second binding
  for the same module, and an edit that would break the file's syntax reverts itself instead
  of being written. `--check` type-checks afterwards with the project's own runtime.
- **`matcha add <sse|stream|buffer>`** — appends a capability handler to the controller,
  idempotent per capability.
- **A `curl | sh` installer** with sha256 verification, and cross-platform binaries attached
  to each release.

[Unreleased]: https://github.com/Expressive-Tea/matcha/compare/v26.8.0-beta.0...HEAD
[26.8.0-beta.0]: https://github.com/Expressive-Tea/matcha/compare/v26.7.0...v26.8.0-beta.0
[26.7.0]: https://github.com/Expressive-Tea/matcha/releases/tag/v26.7.0
