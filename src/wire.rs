//! Pure text-editing core for wiring generated TypeScript symbols into an
//! existing `@green-tea/core` project: adding `import` statements and
//! inserting symbols into a `@Module({...})` array (e.g. `controllers`,
//! `providers`).
//!
//! This module is intentionally free of CLI/I-O concerns — it operates on
//! `&str` in, `String`/`Option<String>` out — so it can be exercised with
//! plain string fixtures and reused by the `add` command (Task 8) without
//! any filesystem coupling.
//!
//! Resolved crate API notes (tree-sitter 0.22.6 / tree-sitter-typescript
//! 0.21.2, confirmed against the installed crate source, not just docs):
//! - The language accessor is the function `tree_sitter_typescript::language_typescript()`
//!   (this crate version does NOT expose a `LANGUAGE_TYPESCRIPT` const).
//! - `Parser::set_language` takes `&Language`.
//! - Grammar node kinds `import_statement`, `pair`, `array` and field names
//!   `key`/`value` on `pair` match the brief's assumptions (verified via
//!   `typescript/src/node-types.json` in the resolved crate).

use tree_sitter::{Node, Parser};

/// Builds a fresh parser configured for the TypeScript grammar.
fn parser() -> Parser {
    let mut p = Parser::new();
    p.set_language(&tree_sitter_typescript::language_typescript())
        .expect("load ts grammar");
    p
}

/// True when tree-sitter parses `src` with no `ERROR`/missing node anywhere
/// in the tree. Used as the revert-on-breakage guarantee: if an edit fails
/// this check, the caller must discard it.
fn parses_clean(src: &str) -> bool {
    let tree = parser().parse(src, None).unwrap();
    !has_error(tree.root_node())
}

fn has_error(node: Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    let mut c = node.walk();
    for child in node.children(&mut c) {
        if has_error(child) {
            return true;
        }
    }
    false
}

/// Wires `symbol` into `src`'s imports from module `from`:
///
/// 1. If an `import { ... } from '<from>';` statement already exists, merge
///    `symbol` into its named-imports brace (`{ A, B } ` → `{ A, B, symbol }`)
///    instead of appending a second, colliding `import` line for the same
///    module (a duplicate ESM binding is a hard `SyntaxError`/`TS2300`, not a
///    harmless dupe). Idempotent: a repeat call with `symbol` already present
///    in that brace returns `src` unchanged.
/// 2. Otherwise, appends `import { <symbol> } from '<from>';` after the last
///    top-level import statement (original behavior, for a module that isn't
///    imported yet at all).
///
/// The brace-merge edit is re-parsed with [`parses_clean`] before being
/// accepted; if merging somehow produces broken text, this falls back to
/// appending a fresh import line rather than returning broken source.
pub fn add_import(src: &str, symbol: &str, from: &str) -> String {
    insert_import(src, symbol, from, false)
}

/// Same as [`add_import`], but for `import type { ... } from '<from>';`.
///
/// A type used in a decorated method signature has to arrive this way. Under
/// `isolatedModules` with `emitDecoratorMetadata` — Deno's defaults — a type
/// reached through a value import is TS1272, because the emitted
/// `design:paramtypes` would reference a binding the compiler erased. Node and
/// bun's `tsc` accept it, so the same generated handler compiles on two
/// runtimes and fails on the third.
pub fn add_type_import(src: &str, symbol: &str, from: &str) -> String {
    insert_import(src, symbol, from, true)
}

fn insert_import(src: &str, symbol: &str, from: &str, type_only: bool) -> String {
    let tree = parser().parse(src, None).unwrap();
    let root = tree.root_node();
    let keyword = if type_only { "import type" } else { "import" };

    if let Some(named_imports) = find_named_imports(&root, src, from, type_only) {
        let mut c = named_imports.walk();
        let already_present = named_imports.named_children(&mut c).any(|spec| {
            spec.kind() == "import_specifier"
                && spec
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(src.as_bytes()).ok())
                    == Some(symbol)
        });
        if already_present {
            return src.to_string();
        }
        let open = named_imports.start_byte();
        let close = named_imports.end_byte() - 1; // the '}'
        let inner = src[open + 1..close].trim().trim_end_matches(',').trim_end();
        let joined = if inner.is_empty() {
            format!(" {symbol} ")
        } else {
            format!(" {inner}, {symbol} ")
        };
        let mut out = String::with_capacity(src.len() + symbol.len() + 2);
        out.push_str(&src[..open + 1]);
        out.push_str(&joined);
        out.push_str(&src[close..]);
        if parses_clean(&out) {
            return out;
        }
        // Merge produced broken text (shouldn't normally happen) — fall
        // through to the append-a-new-line path below rather than return it.
    }

    let line = format!("{keyword} {{ {symbol} }} from '{from}';");
    if src.contains(&line) {
        return src.to_string();
    }
    let mut last_end = 0usize;
    let mut c = root.walk();
    for child in root.children(&mut c) {
        if child.kind() == "import_statement" {
            last_end = child.end_byte();
        }
    }
    if last_end == 0 {
        return format!("{line}\n{src}");
    }
    let mut out = String::with_capacity(src.len() + line.len() + 1);
    out.push_str(&src[..last_end]);
    out.push('\n');
    out.push_str(&line);
    out.push_str(&src[last_end..]);
    out
}

/// Whether an `import_statement` is the `import type { ... }` form.
///
/// Read off the statement's own text rather than its grammar node: the `type`
/// modifier's shape differs between tree-sitter-typescript versions, and the
/// node here is always a well-formed import statement, so the prefix is exact.
fn is_type_only(stmt: &Node, src: &str) -> bool {
    stmt.utf8_text(src.as_bytes())
        .map(|text| text.trim_start().starts_with("import type"))
        .unwrap_or(false)
}

/// Finds the `named_imports` (`{ A, B, C }`) brace of the top-level `import
/// { ... } from '<from>';` statement whose module specifier string literal
/// equals `from` exactly and whose type-only-ness matches `type_only`. Returns
/// `None` when no import statement has that source, or when the matching
/// statement has no named-imports clause at all (e.g. a bare default import) —
/// callers treat that as "can't merge, fall back to appending a new line".
///
/// The `type_only` split matters in both directions: merging a value symbol
/// into an `import type` line erases it at runtime, and merging a type into a
/// value line is the TS1272 that [`add_type_import`] exists to avoid.
fn find_named_imports<'a>(
    root: &Node<'a>,
    src: &str,
    from: &str,
    type_only: bool,
) -> Option<Node<'a>> {
    let mut c = root.walk();
    for stmt in root.children(&mut c) {
        if stmt.kind() != "import_statement" {
            continue;
        }
        if is_type_only(&stmt, src) != type_only {
            continue;
        }
        let Some(source) = stmt.child_by_field_name("source") else {
            continue;
        };
        let Ok(raw) = source.utf8_text(src.as_bytes()) else {
            continue;
        };
        let unquoted = raw
            .strip_prefix(['\'', '"'])
            .and_then(|s| s.strip_suffix(['\'', '"']))
            .unwrap_or(raw);
        if unquoted != from {
            continue;
        }
        let mut sc = stmt.walk();
        for clause in stmt.children(&mut sc) {
            if clause.kind() != "import_clause" {
                continue;
            }
            let mut cc = clause.walk();
            for named in clause.children(&mut cc) {
                if named.kind() == "named_imports" {
                    return Some(named);
                }
            }
        }
    }
    None
}

/// Finds the `key: [ ... ]` array inside the first `@Module({...})` and
/// inserts `symbol` into it, creating `key: [symbol]` inside the module
/// object when that key is absent entirely. Idempotent: a repeat call that
/// would insert an already-present symbol is a no-op. Returns `None` —
/// leaving the caller's original text untouched — when the edited text fails
/// to re-parse clean (an `ERROR`/missing node appears), or when there is no
/// `@Module(...)` object to target at all.
pub fn add_to_module_array(src: &str, key: &str, symbol: &str) -> Option<String> {
    let tree = parser().parse(src, None).unwrap();
    let root = tree.root_node();
    let object = find_module_object(&root, src)?;
    insert_into_object_array(src, object, key, symbol)
}

/// True when `ident` is an identifier anywhere in `src`: an import, a
/// declaration or a use. Read from the parse tree, so a word inside a string
/// (`'@green-tea/core'`) or a comment does not count; those are not bindings
/// and cannot collide.
pub fn binds(src: &str, ident: &str) -> bool {
    fn visit(node: Node, src: &str, ident: &str) -> bool {
        if matches!(
            node.kind(),
            "identifier" | "type_identifier" | "shorthand_property_identifier_pattern"
        ) && node.utf8_text(src.as_bytes()).ok() == Some(ident)
        {
            return true;
        }
        let mut c = node.walk();
        let found = node.children(&mut c).any(|child| visit(child, src, ident));
        found
    }
    let tree = parser().parse(src, None).unwrap();
    visit(tree.root_node(), src, ident)
}

/// Inserts `item` into the `key: [...]` array of the first `createApp(...)`
/// call's argument object, creating the key if absent. `item` is any array
/// element: a symbol for `modules`, a call like `algo()` for `plugins`.
/// Idempotent; returns `None` on parse breakage, when there is no
/// `createApp({...})`, or when `key` holds something other than an array
/// literal.
pub fn add_to_createapp(src: &str, key: &str, item: &str) -> Option<String> {
    let tree = parser().parse(src, None).unwrap();
    let root = tree.root_node();
    let object = find_createapp_object(&root, src)?;
    insert_into_object_array(src, object, key, item)
}

/// `add_to_createapp` for `modules`, kept as the name `create module` reads.
pub fn add_to_createapp_modules(src: &str, symbol: &str) -> Option<String> {
    add_to_createapp(src, "modules", symbol)
}

/// Inserts `symbol` into the `key: [...]` array directly inside `object`
/// (an object-literal node). If that `key` pair is absent, a new
/// `key: [symbol]` pair is inserted into the object. Idempotent; re-parses and
/// returns `None` on breakage.
fn insert_into_object_array(src: &str, object: Node, key: &str, symbol: &str) -> Option<String> {
    if let Some((open, close)) = find_key_array_span(object, src, key) {
        let inner = &src[open + 1..close];
        if inner.split(',').map(str::trim).any(|s| s == symbol) {
            return Some(src.to_string()); // idempotent
        }
        let joined = if inner.trim().is_empty() {
            symbol.to_string()
        } else {
            format!("{}, {}", inner.trim_end().trim_end_matches(','), symbol)
        };
        let mut out = String::with_capacity(src.len() + symbol.len() + 2);
        out.push_str(&src[..open + 1]);
        out.push_str(&joined);
        out.push_str(&src[close..]);
        return if parses_clean(&out) { Some(out) } else { None };
    }

    // The key exists but its value isn't an array literal (e.g. `controllers: SHARED`
    // or `modules: MODULES`). Creating a second `key: [...]` pair parses clean yet
    // silently shadows the user's value at runtime (JS keeps the last duplicate key),
    // so `parses_clean` can't catch it. Refuse — the caller falls back to emit-only.
    if object_has_key(object, src, key) {
        return None;
    }

    // key absent → create `key: [symbol]` inside the object, before its '}'
    let obj_open = object.start_byte();
    let obj_close = object.end_byte() - 1; // the '}'
    let existing = src[obj_open + 1..obj_close].trim();
    let pair = format!("{key}: [{symbol}]");
    let new_inner = if existing.is_empty() {
        format!(" {pair} ")
    } else {
        format!(" {}, {pair} ", existing.trim_end_matches(','))
    };
    let mut out = String::with_capacity(src.len() + pair.len() + 4);
    out.push_str(&src[..obj_open + 1]);
    out.push_str(&new_inner);
    out.push_str(&src[obj_close..]);
    if parses_clean(&out) {
        Some(out)
    } else {
        None
    }
}

/// True when `object` has any direct `pair` child whose key text equals `key`,
/// regardless of the value's kind. Used to avoid creating a duplicate key when
/// an existing pair's value isn't an array literal.
fn object_has_key(object: Node, src: &str, key: &str) -> bool {
    let mut c = object.walk();
    for child in object.named_children(&mut c) {
        if child.kind() == "pair"
            && child
                .child_by_field_name("key")
                .and_then(|k| k.utf8_text(src.as_bytes()).ok())
                == Some(key)
        {
            return true;
        }
    }
    false
}

/// Returns `(open_bracket_byte, close_bracket_byte)` of the `key: [...]` array
/// among `object`'s direct `pair` children, or `None` if absent.
fn find_key_array_span(object: Node, src: &str, key: &str) -> Option<(usize, usize)> {
    let mut c = object.walk();
    for child in object.named_children(&mut c) {
        if child.kind() != "pair" {
            continue;
        }
        let Some(k) = child.child_by_field_name("key") else {
            continue;
        };
        if k.utf8_text(src.as_bytes()).ok() != Some(key) {
            continue;
        }
        let Some(v) = child.child_by_field_name("value") else {
            continue;
        };
        if v.kind() != "array" {
            continue;
        }
        return Some((v.start_byte(), v.end_byte() - 1));
    }
    None
}

/// Locates the argument object literal of the first `createApp(...)` call
/// expression (function identifier `createApp`), in source order.
fn find_createapp_object<'a>(root: &Node<'a>, src: &str) -> Option<Node<'a>> {
    fn visit<'a>(node: Node<'a>, src: &str) -> Option<Node<'a>> {
        if node.kind() == "call_expression" {
            let is_createapp = node
                .child_by_field_name("function")
                .filter(|f| f.kind() == "identifier")
                .and_then(|f| f.utf8_text(src.as_bytes()).ok())
                == Some("createApp");
            if is_createapp {
                if let Some(args) = node.child_by_field_name("arguments") {
                    let mut ac = args.walk();
                    for child in args.named_children(&mut ac) {
                        if child.kind() == "object" {
                            return Some(child);
                        }
                    }
                }
            }
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if let Some(r) = visit(child, src) {
                return Some(r);
            }
        }
        None
    }
    visit(*root, src)
}

/// Inserts `method_src` immediately before the closing `}` of `class
/// <class>`'s body. Returns `None` — leaving the caller's original text
/// untouched — when the edited text fails to re-parse clean, or when no
/// `class <class>` is found at all (the splice point can't be located).
pub fn insert_method(src: &str, class: &str, method_src: &str) -> Option<String> {
    let tree = parser().parse(src, None).unwrap();
    let close = find_class_body_close(&tree.root_node(), src, class)?;
    let mut out = String::with_capacity(src.len() + method_src.len() + 1);
    out.push_str(&src[..close]);
    if !src[..close].ends_with('\n') {
        out.push('\n');
    }
    out.push_str(method_src);
    out.push_str(&src[close..]);
    if parses_clean(&out) {
        Some(out)
    } else {
        None
    }
}

/// Finds the byte offset of the closing `}` of `class <class>`'s body,
/// searching the whole tree in source order (depth-first).
fn find_class_body_close(root: &Node, src: &str, class: &str) -> Option<usize> {
    fn visit(node: Node, src: &str, class: &str) -> Option<usize> {
        if node.kind() == "class_declaration" {
            if let Some(n) = node.child_by_field_name("name") {
                if n.utf8_text(src.as_bytes()).ok() == Some(class) {
                    if let Some(body) = node.child_by_field_name("body") {
                        return Some(body.end_byte() - 1); // the closing '}'
                    }
                }
            }
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if let Some(r) = visit(child, src, class) {
                return Some(r);
            }
        }
        None
    }
    visit(*root, src, class)
}

/// Locates the argument object literal of the first `@Module(...)`
/// decorator in `src`, in source order. A decorator node is recognized as
/// `@Module(...)` when its `call_expression` child has a `function` field
/// that is an `identifier` with text `"Module"`; the object literal is the
/// first `object`-kind named child of that call's `arguments` node.
///
/// Returns `None` when there is no `@Module(...)` decorator at all, or when
/// one exists but its argument isn't an object literal.
fn find_module_object<'a>(root: &Node<'a>, src: &str) -> Option<Node<'a>> {
    fn find_decorator_call<'a>(node: Node<'a>, src: &str) -> Option<Node<'a>> {
        if node.kind() == "decorator" {
            let mut c = node.walk();
            for child in node.children(&mut c) {
                if child.kind() != "call_expression" {
                    continue;
                }
                let is_module = child
                    .child_by_field_name("function")
                    .filter(|f| f.kind() == "identifier")
                    .and_then(|f| f.utf8_text(src.as_bytes()).ok())
                    == Some("Module");
                if is_module {
                    return Some(child);
                }
            }
            // This decorator isn't `@Module(...)`; keep looking elsewhere.
            return None;
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if let Some(r) = find_decorator_call(child, src) {
                return Some(r);
            }
        }
        None
    }

    let call = find_decorator_call(*root, src)?;
    let args = call.child_by_field_name("arguments")?;
    let mut ac = args.walk();
    let object = args
        .named_children(&mut ac)
        .find(|&child| child.kind() == "object");
    object
}

#[cfg(test)]
mod tests {
    #[test]
    fn binds_sees_identifiers_not_strings_or_comments() {
        let src = "import { createApp } from '@green-tea/core';\n// green tea\nexport const app = createApp({});\n";
        assert!(binds(src, "createApp"));
        assert!(binds(src, "app"));
        assert!(!binds(src, "core"));
        assert!(!binds(src, "green"));
        assert!(!binds(src, "appModule"));
    }

    use super::*;

    const MOD: &str = r#"import { Module } from '@green-tea/core';
import { HomeController } from './controllers/home.controller';

@Module({ controllers: [HomeController] })
export class AppModule {}
"#;

    #[test]
    fn import_is_added_once() {
        let out = add_import(MOD, "UsersController", "./controllers/users.controller");
        assert!(out.contains("import { UsersController } from './controllers/users.controller';"));
        let twice = add_import(&out, "UsersController", "./controllers/users.controller");
        assert_eq!(out, twice); // idempotent
    }

    #[test]
    fn symbol_added_to_controllers_array() {
        let out = add_to_module_array(MOD, "controllers", "UsersController").unwrap();
        assert!(out.contains("HomeController"));
        assert!(out.contains("UsersController"));
        // idempotent
        let again = add_to_module_array(&out, "controllers", "UsersController").unwrap();
        assert_eq!(out, again);
    }

    #[test]
    fn broken_result_is_rejected() {
        // Predates create-if-absent: `providers` used to be missing entirely,
        // so the old `find_array` couldn't locate it → None. Now the key is
        // created instead, so either outcome is accepted here; the dedicated
        // `creates_providers_key_and_reverts_on_break` test above covers the
        // create-if-absent behavior explicitly.
        assert!(
            add_to_module_array(MOD, "providers", "X").is_none()
                || add_to_module_array(MOD, "providers", "X")
                    .unwrap()
                    .contains("providers")
        );
    }

    /// Extra coverage beyond the brief: this exercises the `None` path where
    /// `insert_into_object_array` finds (or creates) the target but the
    /// splice produces text that no longer parses clean — proving the
    /// revert-on-ERROR guarantee (`parses_clean` check) is real, not just
    /// theoretically reachable.
    #[test]
    fn revert_on_actual_parse_error() {
        // A "symbol" containing stray syntax breaks the array/object/class
        // once spliced in, so the edited text must fail to re-parse clean.
        let out = add_to_module_array(MOD, "controllers", "X]; class Evil {");
        assert!(out.is_none());
    }

    #[test]
    fn method_inserted_before_class_close() {
        let src = "export class HomeController {\n  home() {}\n}\n";
        let out = insert_method(src, "HomeController", "  @Sse('/zen')\n  zen() {}\n").unwrap();
        assert!(out.contains("zen()"));
        assert!(out.trim_end().ends_with('}'));
    }

    #[test]
    fn targets_module_array_not_a_decoy_before_it() {
        // A decoy object literal with a `controllers:` array appears BEFORE the real @Module.
        let src = r#"const decoy = { controllers: [Existing] };

@Module({ controllers: [HomeController] })
export class AppModule {}
"#;
        let out = add_to_module_array(src, "controllers", "UsersController").unwrap();
        // must land in the @Module array, next to HomeController...
        assert!(out.contains("HomeController, UsersController"));
        // ...and must NOT touch the decoy array
        assert!(out.contains("controllers: [Existing] }"));
    }

    #[test]
    fn insert_method_reverts_on_parse_break() {
        // a method body that makes the class re-parse fail must yield None (revert guard)
        let src = "export class HomeController {\n  home() {}\n}\n";
        assert!(insert_method(src, "HomeController", "  zen() { ]; class Evil {\n").is_none());
    }

    #[test]
    fn insert_method_none_when_class_absent() {
        let src = "export class HomeController {\n  home() {}\n}\n";
        assert!(insert_method(src, "NotHere", "  zen() {}\n").is_none());
    }

    #[test]
    fn add_import_merges_into_existing_brace() {
        let src = "import { Route, Get, Html, Sse } from '@green-tea/core';\n\nexport class C {}\n";
        let out = add_import(src, "Stream", "@green-tea/core");
        assert!(out.contains("Stream"));
        assert_eq!(out.matches("from '@green-tea/core'").count(), 1); // NO duplicate import line
    }

    #[test]
    fn add_import_merge_idempotent_when_symbol_present() {
        let src = "import { Route, Sse } from '@green-tea/core';\nexport class C {}\n";
        assert_eq!(add_import(src, "Sse", "@green-tea/core"), src);
    }

    #[test]
    fn add_import_new_line_when_module_absent() {
        let src = "export class C {}\n";
        let out = add_import(src, "Sse", "@green-tea/core");
        assert!(out.contains("import { Sse } from '@green-tea/core';"));
    }

    #[test]
    fn creates_steps_key_when_absent() {
        // MOD has only `controllers` — adding a step must CREATE `steps: [...]`
        let out = add_to_module_array(MOD, "steps", "PasitoStep").unwrap();
        assert!(out.contains("steps: [PasitoStep]"));
        assert!(out.contains("controllers: [HomeController]")); // untouched
                                                                // idempotent
        assert_eq!(
            add_to_module_array(&out, "steps", "PasitoStep").unwrap(),
            out
        );
    }

    #[test]
    fn creates_providers_key_and_reverts_on_break() {
        let out = add_to_module_array(MOD, "providers", "ConfigProvider").unwrap();
        assert!(out.contains("providers: [ConfigProvider]"));
        // a symbol with stray syntax must still be rejected (revert guard)
        assert!(add_to_module_array(MOD, "providers", "X]; class Evil {").is_none());
    }

    const MAIN: &str = "import { createApp } from '@green-tea/core';\nimport { AppModule } from './app.module';\n\nconst app = createApp({ modules: [AppModule] });\n";

    #[test]
    fn adds_module_to_createapp() {
        let out = add_to_createapp_modules(MAIN, "UsersModule").unwrap();
        assert!(out.contains("AppModule, UsersModule"));
        assert_eq!(add_to_createapp_modules(&out, "UsersModule").unwrap(), out);
        // idempotent
    }

    #[test]
    fn createapp_creates_modules_key_when_absent() {
        let src = "const app = createApp({});\n";
        let out = add_to_createapp_modules(src, "AppModule").unwrap();
        assert!(out.contains("modules: [AppModule]"));
    }

    #[test]
    fn refuses_duplicate_key_when_module_value_not_array() {
        // `controllers` exists but its value is an identifier, not an array literal.
        // Adding must NOT create a second `controllers` pair (silent runtime shadowing).
        let src = "@Module({ mountpoint: '/', controllers: SHARED })\nexport class AppModule {}\n";
        assert!(add_to_module_array(src, "controllers", "X").is_none());
    }

    #[test]
    fn refuses_duplicate_modules_when_createapp_value_not_array() {
        let src = "const app = createApp({ modules: MODULES });\n";
        assert!(add_to_createapp_modules(src, "UsersModule").is_none());
    }

    /// A type in a decorated signature must not ride the value import: under
    /// `isolatedModules` + `emitDecoratorMetadata` that is TS1272, which node's
    /// and bun's tsc accept and Deno rejects.
    #[test]
    fn type_import_gets_its_own_statement() {
        let src = "import { Route } from '@green-tea/core';\n";
        let out = add_type_import(src, "MultipartBody", "@green-tea/core");
        assert!(
            out.contains("import type { MultipartBody } from '@green-tea/core';"),
            "{out}"
        );
        assert!(
            out.contains("import { Route } from '@green-tea/core';"),
            "{out}"
        );
    }

    /// The dangerous direction: a value symbol merged into an `import type`
    /// line is erased at compile time, so the decorator it names is simply
    /// undefined at runtime.
    #[test]
    fn value_import_never_merges_into_a_type_import() {
        let src = "import type { MultipartBody } from '@green-tea/core';\n";
        let out = add_import(src, "Post", "@green-tea/core");
        assert!(
            out.contains("import { Post } from '@green-tea/core';"),
            "{out}"
        );
        assert!(
            !out.contains("import type { MultipartBody, Post }"),
            "Post was merged into the type-only import:\n{out}"
        );
    }

    #[test]
    fn type_import_merges_into_an_existing_type_import() {
        let src = "import type { MultipartBody } from '@green-tea/core';\n";
        let out = add_type_import(src, "UploadedFile", "@green-tea/core");
        assert!(
            out.contains("import type { MultipartBody, UploadedFile }"),
            "{out}"
        );
    }

    #[test]
    fn type_import_is_idempotent() {
        let src = "import type { MultipartBody } from '@green-tea/core';\n";
        let out = add_type_import(src, "MultipartBody", "@green-tea/core");
        assert_eq!(out, src);
    }
}
