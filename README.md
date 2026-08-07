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
```

### The starter

`matcha new <name>` (no flags) scaffolds a live app: it serves an `index.html`
with the green-tea logo and streams a rotating zen message over `@Sse('/zen')`.
Open the browser and it's already alive — `matcha run` boots it.

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

## License

MIT — same as the green-tea framework. Contributions require a DCO sign-off (`git commit -s`).
