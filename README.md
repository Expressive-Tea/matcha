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

Drops the binary in `~/.local/bin`. Prebuilt for macOS (arm64, x86_64) and Linux
(arm64, x86_64), statically linked against musl so any distro works, Alpine
included. Env vars go on the `sh` side of the pipe:

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
matcha create plugin "Plugin Algo"         # an in-app plugin, or --package for its own package
matcha add sse                             # add a capability to your controller
matcha graph                               # draw the dependency graph
matcha explain /users/:id                  # one route's chain, in execution order
matcha openapi                             # structural OpenAPI 3.1 for the route table
matcha doctor                              # check the project for confusing misconfigurations
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
matcha openapi --title "My API" --api-version 1.0.0 --out openapi.json
```

`openapi` is *structural*: green-tea derives paths, methods and parameters from
the decorators, which is everything the graph knows. It does not invent response
schemas, so the document describes the surface rather than the payloads.

Both import `src/app.ts` (override with `--entry`) and call `app.ready()`, which
resolves the graph and deliberately does **not** run provider factories — asking
for a diagram will not open your database connections. Your dependencies have to
be installed, since this runs your code under your runtime.

`explain` takes the route **pattern** as declared, not a concrete URL:
`/users/:id`, not `/users/42`. A miss lists the patterns that are registered.

### `add` capabilities

| | generates |
|---|---|
| `sse` | `@Sse` stream tagged with `sse(data, { id })`, reading `@header('last-event-id')` to resume |
| `ws` | `@Ws` duplex handler: consume `@inbound()`, return a `channel()` — needs `ws` on Node |
| `stream` | `@Stream` chunked response |
| `upload` | `@Post` with `@body(): MultipartBody` — needs `busboy` |
| `buffer` | `@Get` with a custom `@Transformer` shaping the whole response |

`matcha add <cap>` edits the only controller under `src/controllers`; with more
than one it refuses and lists them, and `--controller <path>` picks. Repeats are
no-ops. Both peer dependencies are lazy-loaded by core, so neither is needed to
compile — only to serve the route.

### `create` / `add` auto-wiring

`create` and `add` edit your TypeScript with tree-sitter: they insert imports
and wire pieces into the right `@Module` array or controller. Edits are
idempotent and revert themselves if they would break the file's syntax. Pass
`--check` to `create` to type-check with your project's runtime afterward.

### `matcha create plugin`

An in-app plugin by default, or a package of its own.

```sh
matcha create plugin "Plugin Algo"                                    # plugins/plugin-algo/, registered in createApp
matcha create plugin algo --folder lib/plugins
matcha create plugin algo --package=./algo --scope acme                    # a JSR package
matcha create plugin algo --package=./algo --scope acme --registry both    # JSR + npm, ESM only
```

In-app, each plugin gets a folder of its own under `plugins/` (or `--folder`, relative to the
project root) and is registered in `createApp({ plugins })`. It lives in a folder so it can become
a package later without being pulled out of the app.

`--package` writes a package into the current directory, or `--package=DIR` into `DIR`, and either
one has to be empty. The `=` is required, so a word after a bare `--package` is the plugin's
name. JSR is the default, because it records which runtimes a package supports. `--registry both`
adds npm with a `tsc` build to ESM only, and suggests the `@scope/green-tea-<name>` name without
requiring it. The package passes `deno test`, `node --test`,
`bun test` and `deno publish --dry-run` as generated.

At a terminal it asks for whatever the flags leave out. Without a terminal (CI, a pipe) it takes the
defaults, and for anything that has no default it stops and names the flag to pass.

### `matcha doctor`

The checks whose failures are confusing rather than loud: `experimentalDecorators`
missing (every `@Route` becomes a syntax error, and the message blames the
decorator), a core pin that cannot resolve, an installed version that does not
match the manifest, an entry that does not export `app`, and `@Ws`/multipart
routes whose optional peer dependency is not installed — core lazy-loads `ws`
and `busboy`, so those compile and fail at the first request instead.

Every check is a file read: no runtime is spawned and nothing is fetched, so it
works before the project installs. Exits non-zero on a `✗`.

## Runtime detection

`matcha run` picks the runtime by precedence, first match wins:

1. `matcha.toml` — `runtime = "node" | "deno" | "bun" | "edge"`
2. `wrangler.toml` / `wrangler.jsonc` → **edge**
3. `deno.json` / `deno.jsonc` → **deno**
4. `bun.lockb` / `bun.lock` → **bun**
5. `package.json` → **node**

`wrangler.toml` is checked before `package.json` because an edge project has both.

### Edge

`matcha new my-api --runtime edge` scaffolds a Cloudflare Worker: `wrangler.toml`
with `nodejs_compat`, `edgeHandler(app)` as the default export, and wrangler's
`[assets]` serving `./public` — so `/` is the same page as on the other runtimes
and `/zen` is the same stream, from the worker. `matcha run` is `wrangler dev`,
which boots real workerd locally.

Two differences the edge cannot hide. `@Html('file')`, `static` and multipart
uploads need a filesystem workerd does not have, and shutdown teardown never
runs — an isolate is discarded, not closed. And core is installed from **JSR**
here rather than npm: the npm ESM build carries a `createRequire(import.meta.url)`
banner that throws at load on workerd. The scaffolded `.npmrc` says so and links
[the issue](https://github.com/Expressive-Tea/green-tea/issues/93); both it and
the alias can go once that is fixed.

## Development

```bash
cargo test                          # unit + CLI integration
scripts/starter-smoke.sh node       # scaffold, install, type-check, boot, introspect
scripts/starter-smoke.sh deno       # (also bun, edge) — needs that runtime installed
./test_install.sh                   # install.sh against a file:// fixture, no network
```

`starter-smoke.sh` is the check that `cargo test` cannot be: `cargo test` proves
the CLI writes the files it means to write, not that those files describe a
project the current core will run.

## License

MIT — same as the green-tea framework. Contributions require a DCO sign-off (`git commit -s`).
