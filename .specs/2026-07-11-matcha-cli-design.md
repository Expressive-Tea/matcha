# matcha — green-tea CLI · Design

**Date:** 2026-07-11
**Status:** Approved (brainstorming), pending spec review
**Repo:** `Green-Tea/matcha` (separate repo, Rust — created by Diego)
**Relates to:** `Green-Tea/core` (the green-tea framework)

## 1. What it is

`matcha` is the developer-experience CLI for the green-tea HTTP framework. A
single **native binary written in Rust**, distributed independently of the
framework. It does not require a JS runtime to *install or operate*; it only
*delegates* to the project's runtime when it needs to run or type-check the
project.

**Name rationale:** matcha *is* premium green tea — on-brand, distinctive, no
CLI collision (`gt`=Graphite, `tea`=Gitea's CLI, `gtea`≈gitea were all ruled
out). Crate `matcha`, brew `matcha`, repo `Green-Tea/matcha`.

## 2. Command surface

```
matcha new <name> [--template-url gh:owner/repo]   # scaffold a project
matcha run                                          # detect runtime, run in watch
matcha create <module|controller|step> <Name>       # generate a piece + auto-wire
matcha add <sse|stream|buffer>                       # add a capability to a file
```

`new` scaffolds **projects**; `create` scaffolds **pieces inside** a project.
This split retires the earlier ambiguous `create app`.

## 3. `matcha new`

- No flag → **official `starter` template, embedded in the binary**
  (`include_dir!`), zero network.
- `--template-url gh:owner/repo` → `git clone --depth 1` of the given repo.
  `git` is ubiquitous on dev machines, so the binary carries **no HTTP/TLS
  dependency**. This opens a bring-your-own-template ecosystem while we
  maintain only one official template.
- Token substitution on copy is minimal: `{{project_name}}`. Nothing more.

### The official `starter` (the "wow")

A live app the moment you run it — the create-react-app effect:

- Serves an `index.html` showing the **green-tea logo** (demonstrates
  `@Html` static serving).
- A `@Sse('/zen')` endpoint that streams a **rotating zen message every 30
  seconds** (demonstrates live streaming). The page subscribes via
  `EventSource('/zen')` and updates the zen line under the logo, untouched by
  the user.

Faithful to the real framework API (verified in `core`):

```ts
@Route('/')
class HomeController {
  @Get('/')
  @Html('public/index.html')
  home() {}

  @Sse('/zen')
  zen() {
    const messages = [/* zen lines */];
    return (async function* () {
      for (let i = 0; ; i++) {
        yield { zen: messages[i % messages.length] };
        await new Promise((r) => setTimeout(r, 30_000));
      }
    })();
  }
}
```

Starter layout:

```
<name>/
  src/app.module.ts                 # @Module wiring HomeController
  src/controllers/home.controller.ts
  public/index.html                 # logo + EventSource('/zen')
  <runtime config>                  # deno.json | package.json | ...
  matcha.toml                       # optional runtime override
```

## 4. `matcha run` — runtime detection

Precedence (first match wins):

1. `matcha.toml` `runtime` field — explicit override
2. `deno.json(c)` → **deno**
3. `bun.lock(b)` → **bun**
4. `package.json` → **node**

Runs the project's entry in watch mode by wrapping the detected runtime.
**Edge is not a `run` target in v1** — it deploys, it does not run locally.

## 5. `create` / `add` — codegen

Three distinct jobs, three tools:

- **Edit** → `tree-sitter` + `tree-sitter-typescript` (in-process, no runtime).
  Inserts the `import` and adds the class to the right `@Module` array
  (`controllers` / `providers` / steps) via range-based splice that preserves
  the file's existing formatting byte-for-byte.
- **Syntax check** → re-parse with tree-sitter. If an `ERROR` node appears, the
  edit broke structure → **revert, leave the file untouched**. Free, in-process.
- **Semantic check (optional)** → if a `tsconfig` / `deno.json` is present, run
  `tsc --noEmit` / `deno check` with the detected runtime. Behind `--check`
  (or automatic when config is present). This is the lazy-correct alternative
  to an LSP: an LSP is built for interactive editing, cannot insert into an
  array, and would reintroduce a runtime dependency — a one-shot `tsc`/`deno
  check` gives the same diagnostics with none of the plumbing.

**Idempotent:** if the piece is already registered, do not duplicate.

**Agreed fallback:** if auto-wire proves fragile ("truena como ejote"),
degrade to emit-only — write the file and print "add it to your module."
Auto-wire stays, but is allowed to become opt-in rather than a blocker.

## 6. Architecture (Rust)

- Command parsing: `clap`.
- Templates: `include_dir!` embeds the starter; `--template-url` shells to
  `git clone --depth 1`.
- Codegen: `tree-sitter` + `tree-sitter-typescript`, range-based edits.
- Validation: spawn detected runtime for `tsc --noEmit` / `deno check`.
- Runtime detection: filesystem probes in the precedence order above.

Each command is an isolated unit: parse args → do one job → report. No shared
mutable state between commands.

## 7. Out of scope for v1 (YAGNI)

Template catalog (one starter + BYO url is enough), edge as a `run` target,
plugin system, custom LSP, custom bundler/dev-server.

## 8. Open handoff items

- Diego creates the `Green-Tea/matcha` repo; this spec moves there.
- green-tea logo asset for the starter (`assets/` in `core` has branding).
