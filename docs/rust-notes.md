# Rust Concepts — Index

A running glossary of Rust concepts as they show up in this codebase. Each entry: ELI5 explanation, where we first use it, why it matters here.

---

## Newtype pattern
**First seen:** `crates/lovely-tree/src/types.rs::ElementUuid`

ELI5: a struct with one field that wraps an existing type. `ElementUuid(uuid::Uuid)` *is* a UUID at runtime, but the compiler treats it as a distinct type. Trying to pass an `ElementUuid` where a `PageUuid` is expected is a compile error, even though both are UUIDs underneath.

Why we use it: pages, elements, sessions, users all have UUIDs. Without newtypes, we'd have to read the variable name to know which one a function expects, and a parameter swap would compile fine and silently corrupt data. With newtypes, the type signature is self-documenting and mistakes become compile errors.

The `#[serde(transparent)]` makes the JSON form just a bare UUID string — the wrapper is invisible at the wire level, but visible at the API level.

## Generational keys (slotmap)
**First seen:** `crates/lovely-tree/src/types.rs::NodeId`

ELI5: imagine a `Vec<T>` where indices can't dangle. Each slot has a "generation" counter that bumps when the slot is freed and reused. A `NodeId` carries both a slot index and a generation. If you keep an old `NodeId` around after the node was removed, `tree.get(old_id)` returns `None` — it doesn't silently point at a different node.

Why we use it: the page-element tree mutates a lot in the builder (add, remove, move). We want pointer-like references between nodes (parent, first_child, siblings) without allocating each node on the heap separately, and without `Rc<RefCell<...>>` which is slow and `!Send`. Generational keys give us "safe indexes" — fast like array indices, safe like references.

`slotmap::new_key_type!` is a macro that defines a strongly-typed key. We get one type called `NodeId` for the tree's slot map; another module could define `OtherKey` and the compiler would refuse to mix them.
