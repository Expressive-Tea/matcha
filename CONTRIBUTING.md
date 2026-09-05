# Contributing to matcha

matcha is the [green-tea](https://github.com/Expressive-Tea/green-tea) CLI. It is open
source, but nobody pushes directly — changes land through reviewed pull requests with a
**DCO sign-off**.

## Where to send your changes

Development happens on a private Gitea instance. **GitHub is a downstream mirror**: the
promote workflow pushes `main` and version tags to it and nothing else, so there is no
`develop` there and no long-lived branch for contributions.

**From GitHub:** open your pull request against `main` and say in the description that it
is a contribution rather than a release. A maintainer carries the commits upstream into
Gitea's `develop` with your authorship and your sign-off intact, and the change reaches
GitHub again through the normal promotion. Nothing is merged into the GitHub `main`
directly — that would put the two forges out of sync and break the next release push.

**From Gitea, if you have access:** follow the branch model below.

## Developer Certificate of Origin (DCO)

We use the [Developer Certificate of Origin](./DCO) instead of a CLA — a per-commit
affirmation that you have the right to submit your work under this project's license. No
copyright assignment, no paperwork.

**Every commit must be signed off:**

```bash
git commit -s -m "fix(wire): keep a type import out of the value brace"
```

That appends `Signed-off-by: Your Name <your@email>`, which has to match your commit
author. Forgot it on the last commit:

```bash
git commit --amend -s
git push --force-with-lease
```

## Working with AI assistance

Use one if it helps. There is no permission to ask for and nothing to declare.

What we do ask is that you read what it hands you before it becomes a pull request. Every
line here is reviewed by a person, by hand, and usually that person is one person. A diff
its own author has not read moves that work onto them and turns review into proofreading,
which is the thing review is worst at.

**What assistants tend to get wrong in this repository in particular:**

- **They test the CLI instead of its output.** `cargo test` proves matcha writes the files
  it means to write. It cannot prove those files describe a project that runs. That gap is
  why the templates sat two core releases behind for two months with a green build.
  `scripts/starter-smoke.sh` is the check that closes it; run it for the runtime you touched.
- **They write for Node and assume the rest follows.** matcha scaffolds for node, deno, bun
  and the edge, and the four differ in ways that bite. A generated handler that compiles
  under `tsc` can still be TS1272 under Deno's `isolatedModules` + `emitDecoratorMetadata`.
- **They reach for a crate.** matcha has four dependencies and reads TOML and JSON with
  line scans on purpose. A new dependency for a value that is always written one per line
  is not a trade worth making.
- **They pin `@green-tea/core` with a caret.** Core is prerelease-only, and npm excludes a
  prerelease from any range whose comparators carry a different `major.minor.patch`. A
  caret there is a pin to the oldest matching prerelease that nothing can move off. There
  is a test for this.
- **They delete comments they read as redundant.** A comment here usually carries reasoning
  the code cannot show on its own — why a marker is a route literal rather than a decorator
  name, why the edge scaffold installs from JSR.

**Where you are unsure, say so** — not as a disclosure, as a pointer. "I did not run the
bun leg" tells a reviewer where to spend their attention.

None of this changes the sign-off. `Signed-off-by` says you have the right to submit the
work under this project's license, and that stays true however the text was produced.

## Branch model (GitFlow)

- `main` — production-ready, protected. No direct pushes. Tags are cut here.
- `develop` — active development, protected. Feature branches merge here.
- `feature/<name>` — branch from `develop`.
- `hotfix/<name>` — branch from `main`.
- `release/<version>` — release stabilization.

## Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`,
`chore:`, `docs:`, `test:`, `refactor:`, `ci:`. Focus the message on the *why*. Do not add
AI co-authoring attribution.

Anything user-visible gets a `CHANGELOG.md` entry under `## [Unreleased]`, in the same
voice as the entries around it: what changed, and what it was before.

## Local setup

A stable Rust toolchain is all that is required to build and test the CLI:

```bash
cargo build
cargo test
```

The starter smoke needs the runtime it exercises — node, deno, bun or wrangler (which
`npm install` brings in for the edge scaffold). You are not expected to install all four to
fix a typo; you are expected to run the one your change touches, and to say which in the
pull request.

```bash
cargo build
scripts/starter-smoke.sh node     # also: deno, bun, edge
```

It scaffolds into a temp directory, installs, type-checks, boots, curls `/` and the `/zen`
stream, runs `matcha graph`/`explain`, then adds every `matcha add` capability and
type-checks them together. It picks a random high port, so it does not collide with a
server you already have running; `PORT=4567 scripts/starter-smoke.sh node` overrides.

## Before you open a pull request

Run what CI runs:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
scripts/starter-smoke.sh <the runtime you touched>
```

CI runs the smoke for all four runtimes on a matrix, plus a weekly `core-freshness` job
that fails when `CORE_VERSION` in `src/template.rs` falls behind core's `beta` dist-tag.

**Non-trivial logic leaves a check behind.** Not a suite — the smallest thing that fails if
the logic breaks. A generator gets a test that the generated code carries what it must; a
parser gets a test for the case that motivated it.

## Reporting issues

Bugs and feature requests go to
[GitHub issues](https://github.com/Expressive-Tea/matcha/issues). A report that says what
you ran, what you expected, and what happened — with the runtime and the matcha version
from `matcha --version` — is worth several that do not.

Defects in the framework itself belong in the
[green-tea repository](https://github.com/Expressive-Tea/green-tea/issues), not here.

## Code of conduct

Be decent. Assume good faith, keep criticism about the code, and take the hint when someone
asks you to drop it.
