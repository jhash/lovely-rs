# Rust Concepts — Index

A running glossary of Rust concepts as they show up in this codebase. Each entry: ELI5 explanation, where we first use it, why it matters here.

---

## Newtype pattern
**First seen:** `crates/lovely-tree/src/types.rs::ElementUuid`

ELI5: a struct with one field that wraps an existing type. `ElementUuid(uuid::Uuid)` *is* a UUID at runtime, but the compiler treats it as a distinct type. Trying to pass an `ElementUuid` where a `PageUuid` is expected is a compile error, even though both are UUIDs underneath.

Why we use it: pages, elements, sessions, users all have UUIDs. Without newtypes, we'd have to read the variable name to know which one a function expects, and a parameter swap would compile fine and silently corrupt data. With newtypes, the type signature is self-documenting and mistakes become compile errors.

The `#[serde(transparent)]` makes the JSON form just a bare UUID string — the wrapper is invisible at the wire level, but visible at the API level.

## Declarative macros (`macro_rules!`)
**First seen:** `crates/lovely-tree/src/tags.rs::define_tags!`

ELI5: a macro is a function that runs at compile time, taking *tokens* (pieces of source code) as input and producing more source code. `macro_rules!` is the simpler kind: pattern-match on token shape, expand to other tokens. No types, no logic — just text-shaped substitution with structure.

We define `define_tags! { Div => "div", ... }` and the macro expands to: an enum, a `from_name` match, a `name` match, and a `const ALL: &[Self]` slice — all derived from one list. If we add `Footer => "footer"` to the macro call, all four pieces update automatically. No drift between the parser, the renderer, and the whitelist.

The pattern `$( $variant:ident => $name:literal ),*` says "zero or more pairs separated by commas, each pair being an identifier and a literal." Inside the expansion, `$( ... )*` repeats the body once per matched pair.

When the list grows past what `macro_rules!` can ergonomically express (declarative macros can't read trait impls or call functions), we'd graduate to a procedural macro in a sibling crate. We're nowhere near that.

## `Result<T, E>` and the `?` operator
**First seen:** `crates/lovely-tree/src/attrs.rs::AttrName::new`

ELI5: Rust has no exceptions. Functions that can fail return `Result<T, E>` — either `Ok(value)` or `Err(error)`. Callers must handle both cases (the compiler enforces this).

The `?` operator is "if `Err`, return early from this function with that error converted via `From`." So `let x = thing()?;` desugars to roughly:
```
let x = match thing() { Ok(v) => v, Err(e) => return Err(e.into()) };
```

That `.into()` matters: if the called function returns `Err(InnerError)` and the calling function returns `Result<_, OuterError>`, the conversion happens automatically as long as `impl From<InnerError> for OuterError` exists. We use this throughout — `WebError::from(DbError)`, `DbError::from(sqlx::Error)`, etc.

## Smart-string (`SmolStr`)
**First seen:** `crates/lovely-tree/src/attrs.rs::AttrName`

ELI5: `String` always heap-allocates. `SmolStr` stores up to 23 bytes inline (no heap), and only spills to heap for longer values. Attribute names like `class`, `id`, `data-foo` all fit inline, so we save an allocation per attribute. Cheap win for a hot path.

## Generational keys (slotmap)
**First seen:** `crates/lovely-tree/src/types.rs::NodeId`

ELI5: imagine a `Vec<T>` where indices can't dangle. Each slot has a "generation" counter that bumps when the slot is freed and reused. A `NodeId` carries both a slot index and a generation. If you keep an old `NodeId` around after the node was removed, `tree.get(old_id)` returns `None` — it doesn't silently point at a different node.

Why we use it: the page-element tree mutates a lot in the builder (add, remove, move). We want pointer-like references between nodes (parent, first_child, siblings) without allocating each node on the heap separately, and without `Rc<RefCell<...>>` which is slow and `!Send`. Generational keys give us "safe indexes" — fast like array indices, safe like references.

`slotmap::new_key_type!` is a macro that defines a strongly-typed key. We get one type called `NodeId` for the tree's slot map; another module could define `OtherKey` and the compiler would refuse to mix them.
