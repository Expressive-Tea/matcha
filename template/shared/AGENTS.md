# AGENTS.md — {{project_name}}

A [green-tea](https://green-tea.expressive-tea.io/docs/) app (`@green-tea/core` {{core_version}}), scaffolded by
matcha. Read this before editing.

## green-tea is not Express

green-tea is new, so code that looks right from Express, Fastify or NestJS habits is usually wrong here.

- The request pipeline is a **dependency graph**, not a middleware chain. There is no `app.use`, no
  `next()`, and nothing mutates `req` or `res`.
- A `@Provider` builds a value once at boot. A `@Step` computes a value per request. Each declares
  `provides` and `needs`, and green-tea derives the order from those declarations.
- A handler declares what it needs **in its signature**: `@needs('user')`, `@param('id')`, `@body()`,
  `@query()`, `@cookie('name')`. Return a value to answer: an object becomes JSON 200, `undefined` becomes 204.
  Throw `Unauthorized`, `NotFound` or another `HttpError` to fail the request.
- `createApp` validates the graph at boot. A token nothing provides, a token with two producers, or a node
  that needs what it provides all fail before the app serves anything.
- **Reuse what the graph already has.** Before writing a new step, check whether a token you need, such as
  `user`, already exists. Before writing `@needs('x')`, check that something provides `x`.

## Look before you edit

Ask the app what it contains instead of guessing. These commands read `src/app.ts` without starting a
server or opening connections:

```bash
matcha routes --json              # every route, its transport, and the chain it runs
matcha tokens --json              # every token: who provides it, who needs it
matcha explain /some/:pattern --json   # one route's chain, in execution order
matcha graph --format json        # the whole graph
```

`explain` takes the route **pattern** as declared (`/users/:id`), not a URL.

## Where things are

- `src/app.ts` builds and exports `app` (`createApp({ modules: [...] })`). Introspection and tests import it.
- `src/main.ts` serves `app`. It is the only file that differs per runtime.
- `src/app.module.ts` lists the providers, steps and controllers with `@Module`.
- `matcha create controller|step|provider|module <Name>` generates a piece and wires it into the module.
- `matcha add sse|ws|upload|stream|buffer` adds a capability to a controller.

## Check your work

- Type-check: `npx tsc --noEmit` (Node, edge), `deno check src/main.ts` (Deno), `bunx tsc --noEmit` (Bun).
- Test without a port: `await app.fetch(new Request('http://x/api/thing'))` returns a `Response`.
- `matcha doctor` finds the misconfigurations that fail confusingly.

## The full guide

The rules, decorators, built-in tokens and common mistakes for the installed version are in the package
itself:

- npm / Bun: `node_modules/@green-tea/core/llms.txt`
- Deno (JSR): `https://jsr.io/@green-tea/core/{{core_version}}/llms.txt`

The complete docs as one file: https://green-tea.expressive-tea.io/docs/llms-full.txt
