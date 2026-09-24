//! The text of every file `matcha create plugin` writes. Pure: slug and
//! options in, file content out, so the shapes are tested without a disk.

use crate::naming::{camel, pascal};

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
