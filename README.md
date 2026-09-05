# matcha

The [green-tea](https://github.com/Expressive-Tea/green-tea) CLI — scaffold, run, and generate green-tea apps from a single native binary. No JS runtime required to install or operate.

## Install

```bash
cargo install matcha-cli
```

Builds from [crates.io](https://crates.io/crates/matcha-cli) and installs the `matcha`
binary. Needs a Rust toolchain; takes a few seconds.

**Prebuilt binary** (no Rust), from the latest GitHub release:

```bash
curl -fsSL https://raw.githubusercontent.com/Expressive-Tea/matcha/main/install.sh | sh
```

Drops the binary in `~/.local/bin`. Env vars go on the `sh` side of the pipe:

```bash
curl -fsSL …/install.sh | MATCHA_VERSION=v26.7.0 MATCHA_INSTALL_DIR=/usr/local/bin sh
```

**From git** (compiles the default branch):

```bash
cargo install --git https://github.com/Expressive-Tea/matcha
```

## Usage

```bash
matcha new my-api                          # scaffold from the official starter
matcha new my-api --template-url gh:owner/repo   # scaffold from any git template
matcha run                                 # detect node/deno/bun and run in watch
matcha create controller Users             # generate + auto-wire into @Module
matcha add sse                             # add a capability (sse|stream|buffer) to your controller
matcha graph                               # draw the dependency graph
matcha explain /users/:id                  # one route's chain, in execution order
```

### The starter

`matcha new <name>` (no flags) scaffolds a live app: it serves an `index.html`
with the green-tea logo and streams a rotating zen message over `@Sse('/zen')`.
Open the browser and it's already alive — `matcha run` boots it.

The graph is composed in `src/app.ts`, which exports the app; `src/main.ts`
imports it and binds a port. The split is what lets `matcha graph` and
`matcha explain` read your application without starting a server.

The `@green-tea/core` version is pinned exactly, not with a caret. Core is
prerelease-only, and npm's semver excludes a prerelease from any range whose
comparators carry a different `major.minor.patch` — so `^26.7.0-beta.0` never
resolves past `26.7.0-beta.0` however many betas ship after it. CI scaffolds,
installs, boots and introspects a real starter on every runtime, and a weekly
job fails when the pin falls behind the `beta` dist-tag.

### Introspection

green-tea derives execution order from what each node declares it `needs` and
`provides`, so the graph is a real artifact rather than a diagram someone drew.
`matcha graph` prints it and `matcha explain` prints one route's slice of it.

```bash
matcha graph                        # Mermaid (default)
matcha graph --format dot           # Graphviz
matcha graph --format json          # the raw GraphView
matcha graph --out docs/graph.mmd   # to a file instead of stdout
matcha explain /users/:id           # providers → steps → handler, with needs/provides
```

Both import `src/app.ts` (override with `--entry`) and call `app.ready()`, which
resolves the graph and deliberately does **not** run provider factories — asking
for a diagram will not open your database connections. Your dependencies have to
be installed, since this runs your code under your runtime.

`explain` takes the route **pattern** as declared, not a concrete URL:
`/users/:id`, not `/users/42`. A miss lists the patterns that are registered.

### `create` / `add` auto-wiring

`create` and `add` edit your TypeScript with tree-sitter: they insert imports
and wire pieces into the right `@Module` array or controller. Edits are
idempotent and revert themselves if they would break the file's syntax. Pass
`--check` to `create` to type-check with your project's runtime afterward.

## Runtime detection

`matcha run` picks the runtime by precedence, first match wins:

1. `matcha.toml` — `runtime = "node" | "deno" | "bun"`
2. `deno.json` / `deno.jsonc` → **deno**
3. `bun.lockb` / `bun.lock` → **bun**
4. `package.json` → **node**

Edge is a deploy target, not a `matcha run` target.

## Development

```bash
cargo test                          # unit + CLI integration
scripts/starter-smoke.sh node       # scaffold, install, type-check, boot, introspect
scripts/starter-smoke.sh deno       # (also bun) — needs that runtime installed
```

`starter-smoke.sh` is the check that `cargo test` cannot be: `cargo test` proves
the CLI writes the files it means to write, not that those files describe a
project the current core will run.

## License

MIT — same as the green-tea framework. Contributions require a DCO sign-off (`git commit -s`).
