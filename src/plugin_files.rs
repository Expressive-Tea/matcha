//! The text of every file `matcha create plugin` writes. Pure: slug and
//! options in, file content out, so the shapes are tested without a disk.

use crate::naming::{camel, pascal};
use crate::template::CORE_VERSION;

/// The plugin itself, the same in both modes. It follows the official
/// plugins' convention: one string is the plugin name, the node name and the
/// token, and `provides` renames it, so two instances can sit side by side.
pub fn factory(slug: &str) -> String {
    let fun = camel(slug);
    let opts = format!("{}Options", pascal(slug));
    format!(
        "import type {{ Plugin }} from '@green-tea/core';

export interface {opts} {{
  /** The token this plugin provides. Change it to mount two instances side by side. */
  provides?: string;
}}

export function {fun}(options: {opts} = {{}}): Plugin {{
  const provides = options.provides ?? '{slug}';

  return {{
    name: provides,
    mount({{ scope }}) {{
      scope.add({{
        kind: 'provider',
        name: provides,
        needs: [],
        provides: [provides],
        run: () => ({{ [provides]: null }}),
      }});
    }},
  }};
}}
"
    )
}

pub const JSR_BANNER: &str = "JSR is recommended. It records which runtimes a package supports (Node, Deno, Bun, workerd), and the green-tea plugin listing shows that for every plugin.";

pub struct Package {
    pub slug: String,
    pub scope: String,
    /// `Some(name)` when it also publishes to npm.
    pub npm: Option<String>,
}

impl Package {
    fn jsr_name(&self) -> String {
        format!("@{}/{}", self.scope, self.slug)
    }
}

/// Every file of a package, as (path relative to the package root, content).
pub fn package(p: &Package) -> Vec<(String, String)> {
    let mut files = vec![
        ("deno.json".to_string(), deno_json(p)),
        ("package.json".to_string(), package_json(p)),
        ("src/index.ts".to_string(), factory(&p.slug)),
        (format!("test/{}.test.ts", p.slug), test_ts(p)),
        ("README.md".to_string(), readme(p)),
        (
            "CHANGELOG.md".to_string(),
            "# Changelog\n\n## [Unreleased]\n\n- First version.\n".to_string(),
        ),
        ("LICENSE".to_string(), license(p)),
        (
            ".gitignore".to_string(),
            "node_modules\ndist\n.DS_Store\n".to_string(),
        ),
    ];
    if p.npm.is_some() {
        files.push(("tsconfig.json".to_string(), TSCONFIG.to_string()));
    }
    files
}

fn deno_json(p: &Package) -> String {
    format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "license": "MIT",
  "exports": "./src/index.ts",
  "nodeModulesDir": "auto",
  "imports": {{
    "@green-tea/core": "npm:@green-tea/core@{CORE_VERSION}"
  }},
  "tasks": {{
    "test": "deno test --allow-env --allow-read"
  }},
  "publish": {{
    "include": ["src", "README.md", "CHANGELOG.md", "LICENSE", "deno.json"]
  }}
}}
"#,
        name = p.jsr_name()
    )
}

const ENGINES: &str = r#"  "engines": {
    "node": ">=22.18",
    "deno": ">=2",
    "bun": ">=1.3"
  },"#;

fn package_json(p: &Package) -> String {
    match &p.npm {
        None => format!(
            r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "license": "MIT",
  "type": "module",
{ENGINES}
  "scripts": {{
    "test": "node --test test/*.test.ts"
  }},
  "devDependencies": {{
    "@green-tea/core": "{CORE_VERSION}",
    "@types/node": "^22"
  }}
}}
"#,
            name = p.jsr_name()
        ),
        Some(npm) => format!(
            r#"{{
  "name": "{npm}",
  "version": "0.1.0",
  "license": "MIT",
  "type": "module",
  "exports": {{
    ".": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }}
  }},
  "files": ["dist"],
{ENGINES}
  "scripts": {{
    "build": "tsc",
    "prepublishOnly": "npm run build",
    "test": "node --test test/*.test.ts"
  }},
  "devDependencies": {{
    "@green-tea/core": "{CORE_VERSION}",
    "@types/node": "^22",
    "typescript": "^5"
  }}
}}
"#
        ),
    }
}

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "es2022",
    "module": "nodenext",
    "moduleResolution": "nodenext",
    "declaration": true,
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "skipLibCheck": true
  },
  "include": ["src"]
}
"#;

fn test_ts(p: &Package) -> String {
    let fun = camel(&p.slug);
    let slug = &p.slug;
    format!(
        "import {{ strict as assert }} from 'node:assert';
import {{ test }} from 'node:test';

import {{ createApp }} from '@green-tea/core';

import {{ {fun} }} from '../src/index.ts';

test('mounts, boots, and puts its token in the graph', async () => {{
  const app = createApp({{ modules: [], plugins: [{fun}()] }});
  await app.boot();
  assert.ok(app.graph().nodes.some((node) => node.provides.includes('{slug}')));
}});

test('two instances mount side by side under different tokens', async () => {{
  const app = createApp({{ modules: [], plugins: [{fun}(), {fun}({{ provides: '{slug}-2' }})] }});
  await app.boot();
}});

test('two instances under one name are refused', () => {{
  assert.throws(() => createApp({{ modules: [], plugins: [{fun}(), {fun}()] }}), /two plugins are named/);
}});
"
    )
}

/// MIT as a starting point, which the README tells the author to change. The
/// holder is the scope, the closest thing to an author matcha knows.
fn license(p: &Package) -> String {
    // Whole years since the epoch, averaged over leap years: a copyright line
    // needs the year, not the date, and this needs no dependency.
    let year = 1970
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 31_556_952)
            .unwrap_or(0);
    let scope = &p.scope;
    format!(
        "MIT License

Copyright (c) {year} {scope}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"
    )
}

fn readme(p: &Package) -> String {
    let jsr = p.jsr_name();
    let fun = camel(&p.slug);
    let npm_install = match &p.npm {
        Some(npm) => format!("\nOr from npm:\n\n```bash\nnpm i {npm}\n```\n"),
        None => String::new(),
    };
    format!(
        "# {jsr}

> {JSR_BANNER}

What this plugin adds to a green-tea app, in one or two sentences.

## Runtimes

| Runtime | Supported | Why not |
|---|---|---|
| Node | | |
| Deno | | |
| Bun | | |
| workerd (edge) | | |

Fill in every row. The green-tea plugin listing asks for a reason beside each runtime this does not support.

## Install

```bash
npx jsr add {jsr}
```
{npm_install}
Needs `@green-tea/core@{CORE_VERSION}` or newer.

## Usage

```ts
import {{ createApp }} from '@green-tea/core';
import {{ {fun} }} from '{jsr}';

const app = createApp({{ modules: [AppModule], plugins: [{fun}()] }});
```

## License

MIT, as generated. Change `LICENSE` and the `license` fields in `deno.json` and `package.json`
if you publish under another.

## List it

Once it is published, add it to the green-tea plugin listing: https://green-tea.expressive-tea.io/plugins/#contribute
"
    )
}
