# matcha — multi-runtime starter (node/deno/bun) · Spec

**Date:** 2026-07-12
**Branch:** `feature/matcha-multi-runtime` (off `develop`)
**Goal:** `matcha new <name> --runtime <node|deno|bun>` scaffolds the starter for the chosen runtime. Shared app files are written once; only `main.ts` + runtime config + `matcha.toml` differ per runtime. Default runtime: **node** (green-tea is developed node-first; Node is the reference implementation. deno/bun via `--runtime`).

## Verified framework facts (from Green-Tea/core)
- `@green-tea/core` imports `reflect-metadata` internally (`src/index.ts:6`) — consumers need NO explicit import; it installs transitively with the package.
- Serve per runtime: Node `app.listen(3000)`; Deno `Deno.serve({ port }, app.fetch)`; Bun `Bun.serve({ port, fetch: app.fetch })`. Starter has no WebSocket, so `app.fetch` is sufficient (no `serveDeno`/`serveBun`).
- `@Module` requires `mountpoint`. Package version pin: `^26.7.0-beta.0`.

## New template layout (replaces `template/starter/`)
```
template/
  shared/
    public/index.html
    src/app.module.ts
    src/controllers/home.controller.ts
  runtimes/
    deno/  { matcha.toml, deno.json, src/main.ts }
    node/  { matcha.toml, package.json, tsconfig.json, src/main.ts }
    bun/   { matcha.toml, package.json, tsconfig.json, src/main.ts }
```
`write_starter` writes `shared` then overlays the chosen runtime dir on top (both go into the same project dir; `src/main.ts` from the overlay lands beside `src/app.module.ts` from shared). Token substitution `{{project_name}}` applies to every copied file (now also `package.json` "name").

## Exact file contents

### shared/ (moved verbatim from current `template/starter/`)
- `public/index.html` — unchanged (keep `{{project_name}}` title + `EventSource('/zen')`).
- `src/app.module.ts` — unchanged (`@Module({ mountpoint: '/', controllers: [HomeController] })`).
- `src/controllers/home.controller.ts` — unchanged (`@Route('/')`, `@Get('/') @Html('public/index.html')`, `@Sse('/zen')` rotating zen every 30s).

### runtimes/deno/ (moved from current starter)
- `matcha.toml`: `runtime = "deno"`
- `deno.json` (unchanged from current starter):
```json
{
  "compilerOptions": { "experimentalDecorators": true },
  "unstable": ["sloppy-imports"],
  "imports": { "@green-tea/core": "npm:@green-tea/core@^26.7.0-beta.0" },
  "tasks": { "dev": "deno run --watch --allow-net --allow-read --allow-env src/main.ts" }
}
```
- `src/main.ts`:
```ts
import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

const app = createApp({ modules: [AppModule] });

Deno.serve({ port: 8000 }, app.fetch);
console.log('🍵 green-tea running on http://localhost:8000');
```

### runtimes/node/
- `matcha.toml`: `runtime = "node"`
- `package.json`:
```json
{
  "name": "{{project_name}}",
  "type": "module",
  "private": true,
  "scripts": { "dev": "tsx watch src/main.ts" },
  "dependencies": { "@green-tea/core": "^26.7.0-beta.0" },
  "devDependencies": { "tsx": "^4", "typescript": "^5" }
}
```
- `tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "es2020",
    "module": "esnext",
    "moduleResolution": "bundler",
    "experimentalDecorators": true,
    "strict": true,
    "skipLibCheck": true
  }
}
```
- `src/main.ts`:
```ts
import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

const app = createApp({ modules: [AppModule] });

app.listen(3000);
console.log('🍵 green-tea running on http://localhost:3000');
```

### runtimes/bun/
- `matcha.toml`: `runtime = "bun"`
- `package.json`:
```json
{
  "name": "{{project_name}}",
  "type": "module",
  "private": true,
  "scripts": { "dev": "bun --watch run src/main.ts" },
  "dependencies": { "@green-tea/core": "^26.7.0-beta.0" }
}
```
- `tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "esnext",
    "module": "esnext",
    "moduleResolution": "bundler",
    "experimentalDecorators": true,
    "strict": true,
    "skipLibCheck": true
  }
}
```
- `src/main.ts`:
```ts
import { createApp } from '@green-tea/core';
import { AppModule } from './app.module';

const app = createApp({ modules: [AppModule] });

Bun.serve({ port: 8000, fetch: app.fetch });
console.log('🍵 green-tea running on http://localhost:8000');
```

## Code changes

### `src/template.rs`
- Replace the single `STARTER` embed with `SHARED` + three runtime overlays (`include_dir!` each).
- `pub fn write_starter(dest: &Path, project_name: &str, runtime: Runtime) -> io::Result<()>` — write `SHARED`, then the overlay for `runtime`. Keep `write_dir` substitution logic.

### `src/cli.rs`
- `Command::New` gains `#[arg(long, default_value = "deno", value_parser = ["node", "deno", "bun"])] runtime: String`.

### `src/cmd_new.rs`
- `run(name, template_url, runtime: &str)`: parse `runtime` via `Runtime::from_str` (already in `runtime.rs`); for the starter (no `--template-url`) path, call `write_starter(dest, name, rt)`. `--template-url` path ignores `--runtime`.

### `src/cmd_run.rs`
- Change the Bun mapping from `("bun", vec!["--watch", "run", "dev"])` to `("bun", vec!["run", "dev"])` so all three uniformly run their `dev` script (deno.json/package.json define the watch). Update the `bun_watch` unit test accordingly.

### `src/main.rs`
- Pass the new `runtime` arg through the `Command::New` dispatch.

## Tests
- `template.rs`: parametrize the write test — assert deno writes `deno.json` + `src/main.ts` containing `Deno.serve`; node writes `package.json` (with substituted name) + `tsconfig.json` + `main.ts` containing `app.listen`; bun writes `package.json` + `main.ts` containing `Bun.serve`. All three write the shared `src/app.module.ts` and `public/index.html`.
- `tests/new.rs`: `matcha new demo --runtime node` produces `package.json` with `"name": "demo"` and no `deno.json`; default (no flag) produces `deno.json`.
- `cmd_run.rs`: bun mapping test expects `("bun", vec!["run", "dev"])`.

## Out of scope
Edge starter (deploy target, not `run`). Per-runtime lockfiles. `serveDeno`/`serveBun` (no WS in starter).
