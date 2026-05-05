# Milestone A — Static CMS Slice — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stand up the Rust foundation of lovely-rs — a server-rendered CMS where authenticated users can create and edit pages with elements rendered from Postgres — with the full test pyramid, criterion benches on the tree, and Docker + k8s manifests ready to deploy.

**Architecture:** Cargo workspace of 6 crates: `lovely-tree` (slotmap-backed Page Element DOM), `lovely-db` (sqlx pools + `SqliteAppStore` trait), `lovely-web` (axum router, maud views, OAuth + username/password + TOTP), `lovely-server` (binary glue), `lovely-data` (stub binary), `lovely-test-support` (path-only test helpers). Postgres is the only DB used in milestone A. Per-app SQLite scaffolding lands as the trait + a stub `LocalSqliteAppStore` that errors if called — full impl ships in milestone C.

**Tech Stack:** Rust 1.83, axum 0.8, maud 0.27, sqlx 0.8 (Postgres + SQLite), tokio 1, slotmap 1, oauth2 5, argon2 0.5, totp-rs 5, tower-sessions 0.13, criterion 0.5, testcontainers 0.23, scraper 0.21, fantoccini 0.21 (deferred to milestone B), Postgres 17, Docker (distroless runtime).

**Conventions used in every task:**
- Each step is 2–5 minutes. Each task ends with a test run and a commit.
- All commits are signed off `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>`.
- Comments only when WHY is non-obvious. No `// TODO` without name+date. No file-header banners. Follow the design doc §10.
- After every code change, run `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` and `cargo test --workspace`. If any fails, fix before committing.
- Reference the design doc constantly: `docs/plans/2026-05-05-lovely-rs-design.md`.
- Teaching: when introducing a new Rust concept (see design §10c list), add a one-paragraph entry to `docs/rust-notes.md` linking to the file/line where it first appears.

---

## Phase 0 — Workspace skeleton

### Task 0.1: Create workspace `Cargo.toml` and `rust-toolchain.toml`

**Files:**
- Create: `/Users/jakehash/Development/articles/lovely-rs/Cargo.toml`
- Create: `/Users/jakehash/Development/articles/lovely-rs/rust-toolchain.toml`
- Create: `/Users/jakehash/Development/articles/lovely-rs/.gitignore`

**Step 1: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.83"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

**Step 2: Write workspace `Cargo.toml`** — exactly the workspace block from design doc §11.

```toml
[workspace]
resolver = "2"
members = [
    "crates/lovely-tree",
    "crates/lovely-db",
    "crates/lovely-web",
    "crates/lovely-server",
    "crates/lovely-data",
    "crates/lovely-test-support",
]

[workspace.package]
edition = "2021"
rust-version = "1.83"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

axum = { version = "0.8", features = ["macros", "tracing"] }
axum-extra = { version = "0.10", features = ["cookie", "cookie-signed", "typed-header", "form"] }
axum-htmx = "0.7"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "compression-gzip", "compression-br", "fs", "timeout", "request-id"] }
hyper = "1"

maud = "0.27"

sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "postgres", "sqlite", "uuid", "chrono", "json", "macros", "migrate"] }

oauth2 = "5"
argon2 = "0.5"
totp-rs = { version = "5", features = ["qr"] }
qrcode = "0.14"
jsonwebtoken = "9"
secrecy = { version = "0.10", features = ["serde"] }
rand = "0.8"

tower-sessions = "0.13"
tower-sessions-sqlx-store = { version = "0.14", features = ["postgres"] }

slotmap = "1"
smol_str = "0.3"

dashmap = "6"
moka = { version = "0.12", features = ["future"] }

serde = { version = "1", features = ["derive"] }
serde_json = "1"

uuid = { version = "1", features = ["v4", "v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
clap = { version = "4", features = ["derive", "env"] }
dotenvy = "0.15"

reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "cookies"] }

testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres"] }
scraper = "0.21"
fantoccini = "0.21"
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
```

**Step 3: Write `.gitignore`**

```
/target
**/*.rs.bk
.env
.env.local
.sqlx/
data/
*.swp
.DS_Store
```

**Step 4: Verify** `cargo metadata --format-version=1 > /dev/null` (should fail because no member crates exist yet — that's fine, we just want to confirm the toolchain installs).

Actually: skip step 4. We'll get a real verification once the first crate exists.

**Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore
git commit -m "chore: workspace scaffolding"
```

---

### Task 0.2: Create empty member crates

**Files:**
- Create: `crates/lovely-tree/Cargo.toml`, `crates/lovely-tree/src/lib.rs`
- Create: `crates/lovely-db/Cargo.toml`, `crates/lovely-db/src/lib.rs`
- Create: `crates/lovely-web/Cargo.toml`, `crates/lovely-web/src/lib.rs`
- Create: `crates/lovely-server/Cargo.toml`, `crates/lovely-server/src/main.rs`
- Create: `crates/lovely-data/Cargo.toml`, `crates/lovely-data/src/main.rs`
- Create: `crates/lovely-test-support/Cargo.toml`, `crates/lovely-test-support/src/lib.rs`

**Step 1: Each `Cargo.toml` has the package metadata + an empty `[dependencies]` block.** Example for `lovely-tree`:

```toml
[package]
name = "lovely-tree"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]

[dev-dependencies]
```

Repeat for each crate, adjusting `name`. For `lovely-server` and `lovely-data` add `[[bin]]` blocks pointing at `src/main.rs`.

**Step 2: Each `lib.rs` / `main.rs` is empty (or `fn main() { println!("not yet implemented"); }` for the binaries).**

**Step 3: Verify** `cargo build --workspace`. Expected: success, fast.

**Step 4: Verify** `cargo test --workspace`. Expected: success, 0 tests run.

**Step 5: Commit**

```bash
git add crates/
git commit -m "chore: empty workspace member crates"
```

---

### Task 0.3: Add CI workflow + pre-commit hook scaffold

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.githooks/pre-commit`
- Create: `Makefile`

**Step 1: Write `ci.yml`** with parallel jobs: `fmt`, `clippy`, `test`. Use `dtolnay/rust-toolchain@stable` action. `services: postgres` block on the `test` job (Postgres 17 official image). Set `DATABASE_URL` to `postgres://postgres:postgres@localhost:5432/postgres`.

**Step 2: Write `.githooks/pre-commit`** — bash that runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.

**Step 3: Write `Makefile`** with one target: `hooks: ; git config core.hooksPath .githooks`.

**Step 4: Verify** `make hooks` then `cargo fmt --check`. Expected: success.

**Step 5: Commit**

```bash
git add .github/ .githooks/ Makefile
git commit -m "ci: add GH Actions workflow and pre-commit hook scaffold"
```

---

## Phase 1 — `lovely-tree` core (TDD)

For every task in this phase: **write the test first, run it (must fail), implement, run again (must pass), commit.**

### Task 1.1: `ElementUuid`, `NodeId`, `ElementRow` skeletons

**Files:**
- Modify: `crates/lovely-tree/Cargo.toml` — add deps `slotmap`, `uuid`, `serde`, `serde_json`, `smol_str`, `thiserror`. Dev: `criterion`.
- Create: `crates/lovely-tree/src/lib.rs` (replace empty)
- Create: `crates/lovely-tree/src/types.rs`

**Step 1: Write the failing unit test in `src/lib.rs`:**

```rust
mod types;

#[cfg(test)]
mod tests {
    use super::types::*;

    #[test]
    fn element_uuid_roundtrips_through_string() {
        let u = ElementUuid::new_v4();
        let s = u.to_string();
        let parsed: ElementUuid = s.parse().unwrap();
        assert_eq!(u, parsed);
    }
}
```

**Step 2: Run** `cargo test -p lovely-tree`. Expected: compile error (types module empty).

**Step 3: Implement `types.rs`:**

```rust
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementUuid(pub uuid::Uuid);

impl ElementUuid {
    pub fn new_v4() -> Self { Self(uuid::Uuid::new_v4()) }
    pub fn nil() -> Self { Self(uuid::Uuid::nil()) }
    pub fn into_inner(self) -> uuid::Uuid { self.0 }
}

impl std::fmt::Display for ElementUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for ElementUuid {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self(uuid::Uuid::parse_str(s)?)) }
}

slotmap::new_key_type! { pub struct NodeId; }
```

**Step 4: Run** `cargo test -p lovely-tree`. Expected: 1 passed.

**Step 5: Commit**

```bash
git add crates/lovely-tree/
git commit -m "lovely-tree: add ElementUuid newtype and NodeId slotmap key"
```

---

### Task 1.2: `ElementTag` enum stub + `define_tags!` macro (subset)

**Files:**
- Create: `crates/lovely-tree/src/tags.rs`
- Modify: `crates/lovely-tree/src/lib.rs`

**Step 1: Failing test** in `tags.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_tag() {
        assert_eq!(ElementTag::from_name("div"), Some(ElementTag::Div));
    }

    #[test]
    fn rejects_unknown_tag() {
        assert_eq!(ElementTag::from_name("script"), None);
    }

    #[test]
    fn renders_tag_name() {
        assert_eq!(ElementTag::Section.name(), "section");
    }
}
```

**Step 2: Run** `cargo test -p lovely-tree tags`. Expected: compile error.

**Step 3: Implement** the simplest possible version (no macro yet — just hand-write 3 variants to keep test green):

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ElementTag { Div, Section, Span }

impl ElementTag {
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "div" => Some(Self::Div),
            "section" => Some(Self::Section),
            "span" => Some(Self::Span),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::Section => "section",
            Self::Span => "span",
        }
    }
}
```

**Step 4: Run** `cargo test -p lovely-tree`. Expected: 4 passed.

**Step 5: Now write the `define_tags!` macro.** Add to top of `tags.rs`:

```rust
#[macro_export]
macro_rules! define_tags {
    ( $( $variant:ident => $name:literal ),* $(,)? ) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub enum ElementTag { $( $variant ),* }

        impl ElementTag {
            pub fn from_name(s: &str) -> Option<Self> {
                match s {
                    $( $name => Some(Self::$variant), )*
                    _ => None,
                }
            }
            pub fn name(self) -> &'static str {
                match self {
                    $( Self::$variant => $name, )*
                }
            }
            pub const ALL: &'static [Self] = &[ $( Self::$variant ),* ];
        }
    };
}
```

**Step 6: Replace the hand-written enum** with a macro invocation listing the full v1 tag list (design §6):

```rust
define_tags! {
    Div => "div", Section => "section", Article => "article",
    Header => "header", Footer => "footer", Nav => "nav",
    Main => "main", Aside => "aside",
    H1 => "h1", H2 => "h2", H3 => "h3", H4 => "h4", H5 => "h5", H6 => "h6",
    P => "p", Span => "span", Strong => "strong", Em => "em",
    Blockquote => "blockquote", Code => "code", Pre => "pre",
    A => "a", Ul => "ul", Ol => "ol", Li => "li",
    Img => "img", Figure => "figure", Figcaption => "figcaption",
    Table => "table", Thead => "thead", Tbody => "tbody",
    Tr => "tr", Th => "th", Td => "td",
    Form => "form", Input => "input", Textarea => "textarea",
    Select => "select", Button => "button", Label => "label",
    Hr => "hr", Br => "br",
}
```

(Note: typed payloads for Form/Input/etc. land in Phase 2 when we have the rendering machinery.)

**Step 7: Add a test** that confirms `ElementTag::ALL.len() >= 40` and that `from_name(name())` is identity for all variants:

```rust
#[test]
fn all_tags_roundtrip_through_name() {
    for &tag in ElementTag::ALL {
        assert_eq!(ElementTag::from_name(tag.name()), Some(tag));
    }
}
#[test]
fn rejects_dangerous_tags() {
    for bad in ["script", "iframe", "object", "embed", "style"] {
        assert_eq!(ElementTag::from_name(bad), None, "{} should be rejected", bad);
    }
}
```

**Step 8: Run** `cargo test -p lovely-tree`. Expected: 6 passed.

**Step 9: Add to `docs/rust-notes.md`** an entry: "macro_rules! — declarative macros, see `crates/lovely-tree/src/tags.rs:1`."

**Step 10: Commit**

```bash
git add crates/lovely-tree/ docs/rust-notes.md
git commit -m "lovely-tree: define_tags! macro with v1 whitelist"
```

---

### Task 1.3: `AttrName` validator + `AttrList`

**Files:**
- Create: `crates/lovely-tree/src/attrs.rs`
- Modify: `crates/lovely-tree/src/lib.rs`

**Step 1: Failing tests** in `attrs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_valid_attr_names() {
        for n in ["id", "class", "data-foo", "aria-label", "x-1"] {
            assert!(AttrName::new(n).is_ok(), "{}", n);
        }
    }
    #[test]
    fn rejects_invalid_attr_names() {
        for n in ["", "1foo", "foo bar", "foo<", "on click", "javascript:"] {
            assert!(AttrName::new(n).is_err(), "{}", n);
        }
    }
    #[test]
    fn denies_event_handler_attrs() {
        for n in ["onclick", "onload", "onerror", "onmouseover"] {
            assert!(AttrName::new(n).is_err(), "{} should be denied", n);
        }
    }
    #[test]
    fn denies_htmx_attrs_in_user_provided() {
        for n in ["hx-get", "hx-post", "hx-swap", "hx-target"] {
            assert!(AttrName::new(n).is_err(), "{} should be denied", n);
        }
    }
}
```

**Step 2: Run** the test. Expected: compile error.

**Step 3: Implement** `attrs.rs`:

```rust
use crate::errors::TreeError;
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AttrName(SmolStr);

impl AttrName {
    pub fn new(s: &str) -> Result<Self, TreeError> {
        if s.is_empty() || s.len() > 64 {
            return Err(TreeError::InvalidAttribute(s.into()));
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_alphabetic() {
            return Err(TreeError::InvalidAttribute(s.into()));
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(TreeError::InvalidAttribute(s.into()));
            }
        }
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("on") || lower.starts_with("hx-") {
            return Err(TreeError::InvalidAttribute(s.into()));
        }
        Ok(Self(SmolStr::new(s)))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Clone, Debug, Default)]
pub struct AttrList { entries: Vec<(AttrName, String)> }

impl AttrList {
    pub fn new() -> Self { Self::default() }
    pub fn push(&mut self, name: AttrName, value: impl Into<String>) {
        self.entries.push((name, value.into()));
    }
    pub fn iter(&self) -> impl Iterator<Item = (&AttrName, &str)> {
        self.entries.iter().map(|(n, v)| (n, v.as_str()))
    }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn len(&self) -> usize { self.entries.len() }
}
```

**Step 4: Create `crates/lovely-tree/src/errors.rs`:**

```rust
use crate::types::{ElementUuid, NodeId};

#[derive(thiserror::Error, Debug)]
pub enum TreeError {
    #[error("node {0:?} not found")]
    NotFound(NodeId),
    #[error("uuid {0} not in tree")]
    UnknownUuid(ElementUuid),
    #[error("would create cycle: moving {child:?} into {ancestor:?}")]
    WouldCycle { child: NodeId, ancestor: NodeId },
    #[error("invalid attribute name: {0:?}")]
    InvalidAttribute(String),
    #[error("malformed db row: {0}")]
    MalformedRow(String),
}
```

**Step 5: Wire** `pub mod attrs; pub mod errors; pub mod tags; pub mod types;` and re-exports in `lib.rs`.

**Step 6: Run** `cargo test -p lovely-tree`. Expected: all attrs tests pass.

**Step 7: Add `docs/rust-notes.md`** entry: "newtype + smol_str — see `crates/lovely-tree/src/attrs.rs`."

**Step 8: Commit**

```bash
git add crates/lovely-tree/ docs/rust-notes.md
git commit -m "lovely-tree: AttrName validator, AttrList, TreeError"
```

---

### Task 1.4: `Node` + `Tree::new` + `Tree::get` + `by_uuid` invariant

**Files:**
- Create: `crates/lovely-tree/src/arena.rs`
- Modify: `crates/lovely-tree/src/lib.rs`

**Step 1: Failing tests** in `arena.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::ElementTag;
    use crate::types::ElementUuid;

    #[test]
    fn new_tree_has_root() {
        let root = ElementUuid::new_v4();
        let tree = Tree::new(root, ElementTag::Div);
        assert_eq!(tree.root_uuid(), root);
        assert!(tree.get_by_uuid(root).is_some());
    }
    #[test]
    fn get_by_uuid_returns_none_for_unknown() {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        assert!(tree.get_by_uuid(ElementUuid::new_v4()).is_none());
    }
    #[test]
    fn root_node_has_no_parent_or_siblings() {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let root_id = tree.root();
        let root = tree.get(root_id).unwrap();
        assert_eq!(root.parent, None);
        assert_eq!(root.first_child, None);
        assert_eq!(root.last_child, None);
        assert_eq!(root.prev_sibling, None);
        assert_eq!(root.next_sibling, None);
    }
    #[test]
    fn invariants_hold_on_fresh_tree() {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        tree.debug_assert_invariants();
    }
}
```

**Step 2: Run.** Expected: compile error.

**Step 3: Implement `arena.rs`:**

```rust
use crate::attrs::AttrList;
use crate::errors::TreeError;
use crate::tags::ElementTag;
use crate::types::{ElementUuid, NodeId};
use slotmap::SlotMap;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Node {
    pub uuid: ElementUuid,
    pub tag: ElementTag,
    pub attrs: AttrList,
    pub text: Option<String>,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
}

#[derive(Debug)]
pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    root: NodeId,
    by_uuid: HashMap<ElementUuid, NodeId>,
}

impl Tree {
    pub fn new(root_uuid: ElementUuid, root_tag: ElementTag) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = nodes.insert(Node {
            uuid: root_uuid,
            tag: root_tag,
            attrs: AttrList::new(),
            text: None,
            parent: None, first_child: None, last_child: None,
            prev_sibling: None, next_sibling: None,
        });
        let mut by_uuid = HashMap::new();
        by_uuid.insert(root_uuid, root);
        Self { nodes, root, by_uuid }
    }
    pub fn root(&self) -> NodeId { self.root }
    pub fn root_uuid(&self) -> ElementUuid { self.nodes[self.root].uuid }
    pub fn get(&self, id: NodeId) -> Option<&Node> { self.nodes.get(id) }
    pub fn get_by_uuid(&self, uuid: ElementUuid) -> Option<NodeId> {
        self.by_uuid.get(&uuid).copied()
    }
    pub fn len(&self) -> usize { self.nodes.len() }
    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }

    /// Debug-only invariant check. No-op in release.
    pub fn debug_assert_invariants(&self) {
        if cfg!(debug_assertions) { self.check_invariants().unwrap(); }
    }
    pub(crate) fn check_invariants(&self) -> Result<(), String> {
        // Every uuid in the side-table maps to a live node with that uuid.
        for (uuid, id) in &self.by_uuid {
            let node = self.nodes.get(*id).ok_or_else(|| format!("dangling id {id:?}"))?;
            if node.uuid != *uuid { return Err(format!("uuid mismatch at {id:?}")); }
        }
        // Every live node is indexed by uuid.
        for (id, node) in &self.nodes {
            if self.by_uuid.get(&node.uuid) != Some(&id) {
                return Err(format!("node {id:?} (uuid {}) not in by_uuid", node.uuid));
            }
        }
        Ok(())
    }
}
```

**Step 4: Re-export** from `lib.rs`: `pub use arena::{Node, Tree};`.

**Step 5: Run** `cargo test -p lovely-tree`. Expected: all pass.

**Step 6: Add `docs/rust-notes.md`** entries:
- "`SlotMap` and generational keys — `lovely-tree/src/arena.rs`. ELI5: a `Vec` whose indices can't dangle. When you remove a node, the slot stays but its 'generation' bumps; an old `NodeId` carrying the old generation gets `None` from `get`. Lets us reuse memory safely."
- "Borrowing — `Tree::get(&self)` takes a shared reference; mutators take `&mut self`. The compiler enforces 'one writer or many readers' at zero runtime cost."

**Step 7: Commit**

```bash
git add crates/lovely-tree/ docs/rust-notes.md
git commit -m "lovely-tree: Tree::new, Node, by_uuid invariant"
```

---

### Task 1.5: `append_child`, `insert_before`, `insert_after`

**Files:**
- Modify: `crates/lovely-tree/src/arena.rs`

**Step 1: Failing tests** added to the `tests` module in `arena.rs`:

```rust
#[test]
fn append_child_links_correctly() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let root = tree.root();
    let a = tree.append_child(root, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    let b = tree.append_child(root, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();

    assert_eq!(tree.get(root).unwrap().first_child, Some(a));
    assert_eq!(tree.get(root).unwrap().last_child, Some(b));
    assert_eq!(tree.get(a).unwrap().next_sibling, Some(b));
    assert_eq!(tree.get(b).unwrap().prev_sibling, Some(a));
    assert_eq!(tree.get(a).unwrap().parent, Some(root));
    tree.debug_assert_invariants();
}
#[test]
fn insert_before_inserts_in_middle() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let root = tree.root();
    let a = tree.append_child(root, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    let c = tree.append_child(root, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    let b = tree.insert_before(c, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();

    assert_eq!(tree.get(a).unwrap().next_sibling, Some(b));
    assert_eq!(tree.get(b).unwrap().prev_sibling, Some(a));
    assert_eq!(tree.get(b).unwrap().next_sibling, Some(c));
    assert_eq!(tree.get(c).unwrap().prev_sibling, Some(b));
    tree.debug_assert_invariants();
}
#[test]
fn insert_after_at_end_updates_last_child() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let root = tree.root();
    let a = tree.append_child(root, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    let b = tree.insert_after(a, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    assert_eq!(tree.get(root).unwrap().last_child, Some(b));
    tree.debug_assert_invariants();
}
#[test]
fn append_child_rejects_duplicate_uuid() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let dup = ElementUuid::new_v4();
    tree.append_child(tree.root(), NewNode::new(dup, ElementTag::P)).unwrap();
    let err = tree.append_child(tree.root(), NewNode::new(dup, ElementTag::P)).unwrap_err();
    assert!(matches!(err, TreeError::DuplicateUuid(_)));
}
```

**Step 2: Run.** Expected: compile error (`NewNode`, `DuplicateUuid` missing).

**Step 3: Implement.** Add to `arena.rs`:

```rust
#[derive(Clone, Debug)]
pub struct NewNode {
    pub uuid: ElementUuid,
    pub tag: ElementTag,
    pub attrs: AttrList,
    pub text: Option<String>,
}

impl NewNode {
    pub fn new(uuid: ElementUuid, tag: ElementTag) -> Self {
        Self { uuid, tag, attrs: AttrList::new(), text: None }
    }
}

impl Tree {
    pub fn append_child(&mut self, parent: NodeId, node: NewNode) -> Result<NodeId, TreeError> {
        self.parent_exists(parent)?;
        self.uuid_unused(node.uuid)?;
        let prev = self.nodes[parent].last_child;
        let id = self.insert_node(node, Some(parent), prev, None);
        if let Some(prev) = prev { self.nodes[prev].next_sibling = Some(id); }
        else                     { self.nodes[parent].first_child = Some(id); }
        self.nodes[parent].last_child = Some(id);
        self.debug_assert_invariants();
        Ok(id)
    }

    pub fn insert_before(&mut self, sibling: NodeId, node: NewNode) -> Result<NodeId, TreeError> {
        let target = self.get(sibling).ok_or(TreeError::NotFound(sibling))?;
        let parent = target.parent.ok_or(TreeError::CannotMoveRoot)?;
        let prev = target.prev_sibling;
        self.uuid_unused(node.uuid)?;
        let id = self.insert_node(node, Some(parent), prev, Some(sibling));
        match prev {
            Some(p) => self.nodes[p].next_sibling = Some(id),
            None    => self.nodes[parent].first_child = Some(id),
        }
        self.nodes[sibling].prev_sibling = Some(id);
        self.debug_assert_invariants();
        Ok(id)
    }

    pub fn insert_after(&mut self, sibling: NodeId, node: NewNode) -> Result<NodeId, TreeError> {
        let target = self.get(sibling).ok_or(TreeError::NotFound(sibling))?;
        let parent = target.parent.ok_or(TreeError::CannotMoveRoot)?;
        let next = target.next_sibling;
        self.uuid_unused(node.uuid)?;
        let id = self.insert_node(node, Some(parent), Some(sibling), next);
        match next {
            Some(n) => self.nodes[n].prev_sibling = Some(id),
            None    => self.nodes[parent].last_child = Some(id),
        }
        self.nodes[sibling].next_sibling = Some(id);
        self.debug_assert_invariants();
        Ok(id)
    }

    fn insert_node(&mut self, n: NewNode, parent: Option<NodeId>, prev: Option<NodeId>, next: Option<NodeId>) -> NodeId {
        let uuid = n.uuid;
        let id = self.nodes.insert(Node {
            uuid: n.uuid, tag: n.tag, attrs: n.attrs, text: n.text,
            parent, first_child: None, last_child: None,
            prev_sibling: prev, next_sibling: next,
        });
        self.by_uuid.insert(uuid, id);
        id
    }
    fn parent_exists(&self, id: NodeId) -> Result<(), TreeError> {
        if self.nodes.contains_key(id) { Ok(()) } else { Err(TreeError::NotFound(id)) }
    }
    fn uuid_unused(&self, uuid: ElementUuid) -> Result<(), TreeError> {
        if self.by_uuid.contains_key(&uuid) { Err(TreeError::DuplicateUuid(uuid)) } else { Ok(()) }
    }
}
```

**Step 4: Add new variants to `TreeError`:**

```rust
#[error("duplicate uuid: {0}")]
DuplicateUuid(ElementUuid),
#[error("cannot move root node")]
CannotMoveRoot,
```

**Step 5: Run** `cargo test -p lovely-tree`. Expected: all pass.

**Step 6: Commit**

```bash
git add crates/lovely-tree/
git commit -m "lovely-tree: append_child, insert_before, insert_after"
```

---

### Task 1.6: `remove`, `move_to` with cycle detection, `update`

**Files:**
- Modify: `crates/lovely-tree/src/arena.rs`

**Step 1: Failing tests** added:

```rust
#[test]
fn remove_unlinks_subtree() { /* build a→b→c, remove b, assert a has no children, c gone */ }
#[test]
fn move_to_rejects_cycle() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let a = tree.append_child(tree.root(), NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    let b = tree.append_child(a, NewNode::new(ElementUuid::new_v4(), ElementTag::P)).unwrap();
    let err = tree.move_to(a, b, Position::AppendChild).unwrap_err();
    assert!(matches!(err, TreeError::WouldCycle { .. }));
}
#[test]
fn move_to_into_new_parent_works() { /* a, b children of root; move b to be child of a; verify */ }
#[test]
fn update_changes_attrs_without_restructuring() { /* set attr, read back */ }
```

(Write the bodies fully — don't leave `/* ... */` in the actual code; this plan abbreviates for length.)

**Step 2: Run.** Expected: compile error.

**Step 3: Implement** `Position`, `remove`, `move_to`, `update`. The cycle check for `move_to` walks ancestors of `new_parent` and bails if `id` appears.

```rust
pub enum Position {
    AppendChild,
    PrependChild,
    Before(NodeId),
    After(NodeId),
}

#[derive(Default)]
pub struct NodePatch {
    pub attrs: Option<AttrList>,
    pub text: Option<Option<String>>, // outer Some means "set", None means "leave"
}

impl Tree {
    pub fn remove(&mut self, id: NodeId) -> Result<(), TreeError> {
        if id == self.root { return Err(TreeError::CannotMoveRoot); }
        // Unlink from parent / siblings.
        let node = self.get(id).ok_or(TreeError::NotFound(id))?.clone();
        if let Some(parent) = node.parent {
            if self.nodes[parent].first_child == Some(id) { self.nodes[parent].first_child = node.next_sibling; }
            if self.nodes[parent].last_child  == Some(id) { self.nodes[parent].last_child  = node.prev_sibling; }
        }
        if let Some(p) = node.prev_sibling { self.nodes[p].next_sibling = node.next_sibling; }
        if let Some(n) = node.next_sibling { self.nodes[n].prev_sibling = node.prev_sibling; }
        // Drop subtree.
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            let cur_node = self.nodes.remove(cur).unwrap();
            self.by_uuid.remove(&cur_node.uuid);
            let mut child = cur_node.first_child;
            while let Some(c) = child {
                stack.push(c);
                child = self.nodes.get(c).and_then(|n| n.next_sibling);
            }
        }
        self.debug_assert_invariants();
        Ok(())
    }

    pub fn move_to(&mut self, id: NodeId, new_parent: NodeId, pos: Position) -> Result<(), TreeError> {
        if id == self.root { return Err(TreeError::CannotMoveRoot); }
        // Cycle check: walk ancestors of new_parent.
        let mut a = Some(new_parent);
        while let Some(cur) = a {
            if cur == id { return Err(TreeError::WouldCycle { child: id, ancestor: new_parent }); }
            a = self.nodes.get(cur).and_then(|n| n.parent);
        }
        // Detach (without dropping subtree).
        let original = self.nodes[id].clone();
        if let Some(parent) = original.parent {
            if self.nodes[parent].first_child == Some(id) { self.nodes[parent].first_child = original.next_sibling; }
            if self.nodes[parent].last_child  == Some(id) { self.nodes[parent].last_child  = original.prev_sibling; }
        }
        if let Some(p) = original.prev_sibling { self.nodes[p].next_sibling = original.next_sibling; }
        if let Some(n) = original.next_sibling { self.nodes[n].prev_sibling = original.prev_sibling; }
        self.nodes[id].parent = None;
        self.nodes[id].prev_sibling = None;
        self.nodes[id].next_sibling = None;
        // Re-attach.
        match pos {
            Position::AppendChild => {
                let prev = self.nodes[new_parent].last_child;
                self.nodes[id].parent = Some(new_parent);
                self.nodes[id].prev_sibling = prev;
                if let Some(p) = prev { self.nodes[p].next_sibling = Some(id); }
                else                  { self.nodes[new_parent].first_child = Some(id); }
                self.nodes[new_parent].last_child = Some(id);
            }
            Position::PrependChild => {
                let next = self.nodes[new_parent].first_child;
                self.nodes[id].parent = Some(new_parent);
                self.nodes[id].next_sibling = next;
                if let Some(n) = next { self.nodes[n].prev_sibling = Some(id); }
                else                  { self.nodes[new_parent].last_child = Some(id); }
                self.nodes[new_parent].first_child = Some(id);
            }
            Position::Before(sibling) => {
                let parent = self.nodes[sibling].parent.ok_or(TreeError::CannotMoveRoot)?;
                let prev = self.nodes[sibling].prev_sibling;
                self.nodes[id].parent = Some(parent);
                self.nodes[id].prev_sibling = prev;
                self.nodes[id].next_sibling = Some(sibling);
                self.nodes[sibling].prev_sibling = Some(id);
                match prev { Some(p) => self.nodes[p].next_sibling = Some(id),
                             None    => self.nodes[parent].first_child = Some(id) }
            }
            Position::After(sibling) => {
                let parent = self.nodes[sibling].parent.ok_or(TreeError::CannotMoveRoot)?;
                let next = self.nodes[sibling].next_sibling;
                self.nodes[id].parent = Some(parent);
                self.nodes[id].prev_sibling = Some(sibling);
                self.nodes[id].next_sibling = next;
                self.nodes[sibling].next_sibling = Some(id);
                match next { Some(n) => self.nodes[n].prev_sibling = Some(id),
                             None    => self.nodes[parent].last_child = Some(id) }
            }
        }
        self.debug_assert_invariants();
        Ok(())
    }

    pub fn update(&mut self, id: NodeId, patch: NodePatch) -> Result<(), TreeError> {
        let n = self.nodes.get_mut(id).ok_or(TreeError::NotFound(id))?;
        if let Some(a) = patch.attrs { n.attrs = a; }
        if let Some(t) = patch.text  { n.text = t; }
        Ok(())
    }
}
```

**Step 4: Run.** Expected: all pass.

**Step 5: Commit**

```bash
git add crates/lovely-tree/
git commit -m "lovely-tree: remove, move_to (cycle-safe), update"
```

---

### Task 1.7: Iterators (`children`, `ancestors`, `descendants`)

**Files:**
- Create: `crates/lovely-tree/src/iter.rs`
- Modify: `crates/lovely-tree/src/lib.rs`

**Step 1: Failing tests** in `iter.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn children_iterates_in_order() { /* root with a,b,c — collect children gives [a,b,c] */ }
    #[test]
    fn ancestors_walks_to_root() { /* a > b > c — ancestors(c) = [b, a] */ }
    #[test]
    fn descendants_is_preorder() { /* root > [a > [b], c] — descendants = [a, b, c] */ }
    #[test]
    fn iterators_are_lazy() { /* take(1) on a million-node tree finishes fast */ }
}
```

**Step 2: Run.** Expected: compile error.

**Step 3: Implement** `ChildrenIter`, `AncestorsIter`, `DescendantsIter`. These are zero-copy lazy iterators over `&Tree`.

**Step 4: Run.** Expected: pass.

**Step 5: Add `docs/rust-notes.md` entry**: "Iterators and lifetimes — `lovely-tree/src/iter.rs`. ELI5: an iterator borrows the tree (`&'a Tree`) and remembers where it is. The `'a` says 'I'm only valid while you don't mutate the tree.' Compiler stops you from holding both an iterator and an `&mut self` at the same time."

**Step 6: Commit**

```bash
git add crates/lovely-tree/ docs/rust-notes.md
git commit -m "lovely-tree: lazy children/ancestors/descendants iterators"
```

---

### Task 1.8: `from_db_rows` builder

**Files:**
- Create: `crates/lovely-tree/src/build.rs`
- Modify: `crates/lovely-tree/src/lib.rs`

**Step 1: Failing test:**

```rust
#[test]
fn builds_tree_from_rows_in_any_input_order() {
    // 5 rows: root, a (child of root), b (child of root, after a),
    // c (child of a), d (child of root, after b).
    // Submit in shuffled order. Resulting tree should match canonical order.
}
#[test]
fn rejects_rows_with_no_root() { /* all rows have parent_id Some — error */ }
#[test]
fn rejects_rows_with_two_roots() { /* two rows with parent_id None — error */ }
#[test]
fn rejects_rows_with_orphan() { /* row references parent_id not in set — error */ }
#[test]
fn rejects_rows_with_cycle() { /* a's parent = b, b's parent = a — error */ }
```

**Step 2: Run.** Expected: compile error.

**Step 3: Implement** `ElementRow` struct + `Tree::from_db_rows`:

```rust
#[derive(Clone, Debug)]
pub struct ElementRow {
    pub id: ElementUuid,
    pub parent_id: Option<ElementUuid>,
    pub prev_sibling: Option<ElementUuid>,
    pub tag: String,
    pub attrs_json: serde_json::Value,
    pub text: Option<String>,
}

impl Tree {
    pub fn from_db_rows(rows: &[ElementRow]) -> Result<Self, TreeError> {
        // 1) find unique root (parent_id = None)
        // 2) build adjacency via parent_id, then sort siblings by walking prev_sibling chain
        // 3) cycle check: BFS from root must visit all rows
        // 4) construct in pre-order via append_child
    }
}
```

(Full impl: ~80 lines. Build a `HashMap<ElementUuid, &ElementRow>`, find the root, BFS-construct.)

**Step 4: Run.** Expected: all pass.

**Step 5: Commit**

```bash
git add crates/lovely-tree/
git commit -m "lovely-tree: from_db_rows with cycle/orphan validation"
```

---

### Task 1.9: Maud `Render` impl behind `render` feature

**Files:**
- Modify: `crates/lovely-tree/Cargo.toml` — add optional `maud`, define `render` feature.
- Create: `crates/lovely-tree/src/render.rs`
- Modify: `crates/lovely-tree/src/lib.rs`

**Step 1: Cargo additions:**

```toml
[dependencies]
maud = { workspace = true, optional = true }

[features]
default = []
render = ["dep:maud"]
```

**Step 2: Failing test** in `render.rs` (gated `#[cfg(feature = "render")]`):

```rust
#[test]
fn renders_simple_div_with_attr() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let mut attrs = AttrList::new();
    attrs.push(AttrName::new("class").unwrap(), "container");
    let mut new_node = NewNode::new(ElementUuid::new_v4(), ElementTag::P);
    new_node.attrs = attrs;
    new_node.text = Some("hello".into());
    tree.append_child(tree.root(), new_node).unwrap();
    let html = tree.render().into_string();
    assert!(html.starts_with("<div"));
    assert!(html.contains("<p class=\"container\">hello</p>"));
    assert!(html.ends_with("</div>"));
}
#[test]
fn escapes_text_content() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let mut n = NewNode::new(ElementUuid::new_v4(), ElementTag::P);
    n.text = Some("<script>alert(1)</script>".into());
    tree.append_child(tree.root(), n).unwrap();
    let html = tree.render().into_string();
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
}
#[test]
fn iterative_render_does_not_overflow_on_deep_tree() {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let mut parent = tree.root();
    for _ in 0..10_000 {
        parent = tree.append_child(parent, NewNode::new(ElementUuid::new_v4(), ElementTag::Div)).unwrap();
    }
    let _ = tree.render(); // must not stack-overflow
}
```

**Step 3: Run** `cargo test -p lovely-tree --features render`. Expected: compile error.

**Step 4: Implement** `render.rs`:

```rust
use crate::{Tree, NodeId};
use maud::{Markup, PreEscaped, Render};

impl Tree {
    pub fn render(&self) -> Markup { self.render_subtree(self.root()) }
    pub fn render_subtree(&self, root: NodeId) -> Markup {
        let mut out = String::new();
        render_iter(self, root, &mut out);
        PreEscaped(out)
    }
}

fn render_iter(tree: &Tree, root: NodeId, out: &mut String) {
    enum Step { Open(NodeId), Close(&'static str) }
    let mut stack: Vec<Step> = Vec::with_capacity(64);
    stack.push(Step::Open(root));
    while let Some(step) = stack.pop() {
        match step {
            Step::Open(id) => {
                let node = match tree.get(id) { Some(n) => n, None => continue };
                let tag = node.tag.name();
                out.push('<'); out.push_str(tag);
                for (name, value) in node.attrs.iter() {
                    out.push(' '); out.push_str(name.as_str());
                    out.push_str("=\""); push_escaped(value, out); out.push('"');
                }
                if is_void(node.tag) { out.push_str(" />"); continue; }
                out.push('>');
                if let Some(t) = &node.text { push_escaped(t, out); }
                stack.push(Step::Close(tag));
                // Push children in reverse so we visit first_child first when popping.
                let mut children: Vec<NodeId> = Vec::new();
                let mut c = node.first_child;
                while let Some(cid) = c {
                    children.push(cid);
                    c = tree.get(cid).and_then(|n| n.next_sibling);
                }
                for c in children.into_iter().rev() { stack.push(Step::Open(c)); }
            }
            Step::Close(tag) => { out.push_str("</"); out.push_str(tag); out.push('>'); }
        }
    }
}

fn is_void(tag: crate::tags::ElementTag) -> bool {
    use crate::tags::ElementTag::*;
    matches!(tag, Img | Br | Hr | Input)
}

fn push_escaped(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&'  => out.push_str("&amp;"),
            '<'  => out.push_str("&lt;"),
            '>'  => out.push_str("&gt;"),
            '"'  => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _    => out.push(c),
        }
    }
}
```

**Step 5: Run** `cargo test -p lovely-tree --features render`. Expected: all pass.

**Step 6: Add `docs/rust-notes.md` entry**: "Cargo features — `lovely-tree/Cargo.toml`. ELI5: an opt-in compile flag. `lovely-tree` doesn't depend on `maud` by default; turning on `render` enables the impl. Lets `lovely-server` request it (`lovely-tree = { …, features = ["render"] }`) without forcing maud on benchmark builds."

**Step 7: Commit**

```bash
git add crates/lovely-tree/ docs/rust-notes.md
git commit -m "lovely-tree: iterative maud Render behind 'render' feature"
```

---

### Task 1.10: Criterion benchmarks

**Files:**
- Create: `crates/lovely-tree/benches/tree.rs`
- Modify: `crates/lovely-tree/Cargo.toml` — add `[[bench]]` block, `harness = false`.

**Step 1: Cargo additions:**

```toml
[[bench]]
name = "tree"
harness = false

[dev-dependencies]
criterion = { workspace = true }
```

**Step 2: Implement `benches/tree.rs`:**

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lovely_tree::*;

fn bench_build_from_rows_1k(c: &mut Criterion) {
    let rows = generate_rows(1_000);
    c.bench_function("build_from_rows_1k", |b| b.iter(|| {
        let _t = Tree::from_db_rows(black_box(&rows)).unwrap();
    }));
}
fn bench_find_by_uuid(c: &mut Criterion) {
    let (tree, sample) = build_tree_with_sample(1_000);
    c.bench_function("find_by_uuid", |b| b.iter(|| {
        for u in &sample { black_box(tree.get_by_uuid(*u)); }
    }));
}
fn bench_insert_in_1k(c: &mut Criterion) { /* repeatedly insert at random spots */ }
fn bench_remove_in_1k(c: &mut Criterion) { /* remove random subtrees */ }
fn bench_render_full_1k(c: &mut Criterion) { /* render 1k-node tree */ }
fn bench_render_subtree_depth_10(c: &mut Criterion) { /* render a 10-deep subtree */ }

criterion_group!(benches,
    bench_build_from_rows_1k, bench_find_by_uuid,
    bench_insert_in_1k, bench_remove_in_1k,
    bench_render_full_1k, bench_render_subtree_depth_10);
criterion_main!(benches);

fn generate_rows(n: usize) -> Vec<ElementRow> { /* random valid tree */ }
fn build_tree_with_sample(n: usize) -> (Tree, Vec<ElementUuid>) { /* ... */ }
```

**Step 3: Run** `cargo bench -p lovely-tree --features render -- --warm-up-time 1 --measurement-time 2`. Expected: numbers print. Save as baseline:

```bash
cargo bench -p lovely-tree --features render -- --save-baseline initial
```

**Step 4: Commit**

```bash
git add crates/lovely-tree/
git commit -m "lovely-tree: criterion benches with initial baseline"
```

---

### Task 1.11: Public API docs + invariant property tests

**Files:**
- Modify: every `pub` item in `lovely-tree` to add `///` doc.
- Create: `crates/lovely-tree/tests/invariants.rs`

**Step 1: Add docstrings.** One paragraph per non-trivial pub item, one line for trivial.

**Step 2: Add property tests.** Sequence of randomized ops (`append_child`, `insert_before`, `move_to`, `remove`) — after each, call `tree.check_invariants().unwrap()`. Use a small handwritten loop, not `proptest` (one less dep).

**Step 3: Run** `cargo test -p lovely-tree --all-features`. Expected: all pass.

**Step 4: Run** `cargo doc -p lovely-tree --no-deps`. Expected: success, no warnings.

**Step 5: Commit**

```bash
git add crates/lovely-tree/
git commit -m "lovely-tree: pub docs and invariant property tests"
```

---

## Phase 2 — `lovely-db` Postgres layer

### Task 2.1: Postgres migrations files

**Files:**
- Create: `migrations/20260505000001_users.up.sql` (+ `.down.sql`)
- Create: `migrations/20260505000002_oauth_identities.up.sql` (+ down)
- Create: `migrations/20260505000003_sessions.up.sql` (+ down)
- Create: `migrations/20260505000004_pages.up.sql` (+ down)
- Create: `migrations/20260505000005_elements.up.sql` (+ down)

**Step 1: Write each `.up.sql`** exactly per design §4.

**Step 2: Write each `.down.sql`** with the inverse `DROP TABLE` etc. (in reverse order of dependencies).

**Step 3: Verify locally.** Boot Postgres via `docker compose`:
```bash
docker run --rm -d --name lovely-pg -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:17
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo install sqlx-cli --no-default-features --features postgres,rustls
sqlx migrate run
sqlx migrate revert --target-version 0  # ensure down chain is clean
sqlx migrate run                         # re-apply
```

**Step 4: Commit**

```bash
git add migrations/
git commit -m "db: initial Postgres migrations (users, oauth, sessions, pages, elements)"
```

---

### Task 2.2: `lovely-db` Cargo deps + module skeleton

**Files:**
- Modify: `crates/lovely-db/Cargo.toml`
- Modify: `crates/lovely-db/src/lib.rs`
- Create: `crates/lovely-db/src/errors.rs`, `pg.rs`, `sqlite_store.rs`

**Step 1: Cargo:**

```toml
[dependencies]
sqlx = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
dashmap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
lovely-tree = { path = "../lovely-tree" }

[dev-dependencies]
testcontainers = { workspace = true }
testcontainers-modules = { workspace = true }
tempfile = { workspace = true }
```

**Step 2: Write `errors.rs`** exactly per design §4 (`DbError`).

**Step 3: Write `pg.rs`** with a `connect(url) -> Result<PgPool>` helper that creates a pool with `max_connections = 16` and runs `sqlx::migrate!("../../migrations").run(&pool)`.

**Step 4: Write `sqlite_store.rs`** — just the trait for now:

```rust
use crate::errors::DbError;
use uuid::Uuid;

pub type AppId = Uuid;

#[async_trait::async_trait]
pub trait SqliteAppStore: Send + Sync + 'static {
    async fn get_pool(&self, app_id: AppId) -> Result<sqlx::SqlitePool, DbError>;
    async fn ensure_migrated(&self, app_id: AppId) -> Result<(), DbError>;
    async fn close_pool(&self, app_id: AppId) -> Result<(), DbError>;
    async fn delete_app(&self, app_id: AppId) -> Result<(), DbError>;
}

/// Stub for milestone A — errors on every call. Real impl ships in milestone C.
pub struct StubSqliteAppStore;

#[async_trait::async_trait]
impl SqliteAppStore for StubSqliteAppStore {
    async fn get_pool(&self, _: AppId) -> Result<sqlx::SqlitePool, DbError> {
        Err(DbError::AppNotFound(Uuid::nil()))
    }
    async fn ensure_migrated(&self, _: AppId) -> Result<(), DbError> { Ok(()) }
    async fn close_pool(&self, _: AppId) -> Result<(), DbError> { Ok(()) }
    async fn delete_app(&self, _: AppId) -> Result<(), DbError> { Ok(()) }
}
```

**Step 5: `lib.rs` re-exports:**

```rust
pub mod errors;
pub mod pg;
pub mod sqlite_store;
pub use errors::DbError;
pub use sqlite_store::{AppId, SqliteAppStore, StubSqliteAppStore};
```

**Step 6: Verify** `cargo build -p lovely-db`. Expected: success.

**Step 7: Commit**

```bash
git add crates/lovely-db/
git commit -m "lovely-db: deps, errors, pg::connect, SqliteAppStore trait + stub"
```

---

### Task 2.3: Test harness for testcontainers Postgres

**Files:**
- Create: `crates/lovely-test-support/src/lib.rs`

**Step 1: Cargo deps for `lovely-test-support`:**

```toml
[dependencies]
testcontainers = { workspace = true }
testcontainers-modules = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true }
uuid = { workspace = true }
anyhow = { workspace = true }
```

**Step 2: Implement** a `PgTestContainer` helper:

```rust
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PgTestContainer {
    _container: ContainerAsync<Postgres>,
    pub admin_url: String,
}

impl PgTestContainer {
    pub async fn start() -> anyhow::Result<Self> {
        let container = Postgres::default().with_tag("17").start().await?;
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(5432).await?;
        let admin_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);
        Ok(Self { _container: container, admin_url })
    }
    pub async fn fresh_db(&self) -> anyhow::Result<PgPool> {
        let admin = PgPool::connect(&self.admin_url).await?;
        let dbname = format!("test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE \"{}\"", dbname)).execute(&admin).await?;
        let url = self.admin_url.replace("/postgres", &format!("/{}", dbname));
        let pool = PgPool::connect(&url).await?;
        sqlx::migrate!("../../migrations").run(&pool).await?;
        Ok(pool)
    }
}
```

**Step 3: Failing integration test** in `lovely-db/tests/pg_smoke.rs`:

```rust
#[tokio::test]
async fn migrations_apply_and_users_table_exists() {
    let pg = lovely_test_support::PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, 0);
}
```

**Step 4: Run** `cargo test -p lovely-db --test pg_smoke`. Expected: pass (Docker required).

**Step 5: Commit**

```bash
git add crates/lovely-db/ crates/lovely-test-support/
git commit -m "test: PgTestContainer + smoke test for Postgres migrations"
```

---

### Task 2.4: User repo (CRUD + lookup helpers)

**Files:**
- Create: `crates/lovely-db/src/users.rs`
- Modify: `crates/lovely-db/src/lib.rs`

**Step 1: Failing tests** in `crates/lovely-db/tests/users.rs`:

```rust
#[tokio::test]
async fn create_and_find_user_by_username() { /* ... */ }
#[tokio::test]
async fn username_uniqueness_enforced() { /* ... */ }
#[tokio::test]
async fn email_optional_and_unique_when_present() { /* ... */ }
#[tokio::test]
async fn find_by_oauth_identity_creates_or_returns_user() { /* ... */ }
```

**Step 2: Run.** Expected: compile error.

**Step 3: Implement `users.rs`** with `User` struct, `create_user`, `find_user_by_username`, `find_user_by_id`, `find_or_create_oauth_user`, `set_password_hash`, `set_totp_secret`. All `async fn` taking `&PgPool`. All return `Result<_, DbError>`. Use `sqlx::query_as!` with the typed struct.

**Step 4: Run** all tests. Expected: pass.

**Step 5: Add `docs/rust-notes.md` entry**: "Async + tokio — `lovely-db/src/users.rs`. ELI5: `async fn foo() -> T` returns a `Future<Output = T>`. Doesn't run until awaited. `tokio` is the executor that drives the futures. Every DB call is non-blocking — the thread can serve other requests while waiting on the network."

**Step 6: Commit**

```bash
git add crates/lovely-db/ docs/rust-notes.md
git commit -m "lovely-db: users repo with sqlx query_as!"
```

---

### Task 2.5: OAuth identities repo

**Files:**
- Create: `crates/lovely-db/src/oauth.rs`
- Test: `crates/lovely-db/tests/oauth.rs`

**Step 1: Failing tests:** `upsert_oauth_identity_returns_existing_user`, `upsert_oauth_identity_creates_new_user_when_first_seen`, `unique_constraint_on_provider_provider_user_id`.

**Step 2: Implement** `OAuthIdentity` struct, `upsert_oauth_identity(provider, provider_user_id, raw_profile)` returning `(User, OAuthIdentity)`. Inside one transaction.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-db/
git commit -m "lovely-db: oauth_identities upsert"
```

---

### Task 2.6: Sessions repo

**Files:**
- Create: `crates/lovely-db/src/sessions.rs`
- Test: `crates/lovely-db/tests/sessions.rs`

**Step 1: Failing tests:** `create_session`, `find_session_by_id`, `expired_sessions_are_not_returned`, `delete_session`, `delete_all_sessions_for_user`.

**Step 2: Implement.** Session ID is 256 random bits, base64url. CSRF token same. `find_session_by_id` returns the row if `expires_at > now()`, else `None`.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-db/
git commit -m "lovely-db: sessions repo (create, find, expire, revoke)"
```

---

### Task 2.7: Pages + elements repo

**Files:**
- Create: `crates/lovely-db/src/pages.rs`
- Create: `crates/lovely-db/src/elements.rs`
- Test: `crates/lovely-db/tests/pages_elements.rs`

**Step 1: Failing tests:**

- `create_page_with_root_element`
- `find_page_by_slug`
- `slug_uniqueness_enforced`
- `list_published_pages`
- `update_page_metadata`
- `delete_page_cascades_elements`
- `load_elements_for_page_returns_rows_in_arbitrary_order` (we sort in `lovely-tree`)
- `load_elements_for_page_yields_valid_tree` (call `Tree::from_db_rows` on the result)
- `insert_element_under_parent_with_prev_sibling_link`
- `update_element_attrs`
- `delete_element_cascades_descendants`

**Step 2: Implement `pages.rs`** — `Page` struct, `create_page`, `find_page_by_slug`, etc. `create_page` is a transaction: inserts a row in `pages` (with `root_element = NULL`), inserts the root element (parent_id NULL) referencing the page, then `UPDATE pages SET root_element = $1`.

**Step 3: Implement `elements.rs`** — `ElementRow` mirrors what `lovely-tree` expects. `load_elements_for_page` returns `Vec<lovely_tree::ElementRow>` ready for `Tree::from_db_rows`. `insert_element` and `delete_element` mutate the linked-list pointers (`prev_sibling`) inside a transaction.

**Step 4: Run.** Expected: all pass.

**Step 5: Commit**

```bash
git add crates/lovely-db/
git commit -m "lovely-db: pages and elements repos with linked-list ordering"
```

---

## Phase 3 — `lovely-web` foundation

### Task 3.1: Cargo deps + module skeleton

**Files:**
- Modify: `crates/lovely-web/Cargo.toml`
- Modify: `crates/lovely-web/src/lib.rs`
- Create: `crates/lovely-web/src/{router,errors,state}.rs`

**Step 1: Add deps** per design §11. `lovely-tree = { path = "../lovely-tree", features = ["render"] }`.

**Step 2: Write `errors.rs`** — `WebError` enum + `IntoResponse` impl per design §14. The `IntoResponse` checks `HX-Request` header and either returns `HX-Redirect` for htmx auth failures or a plain redirect/page.

**Step 3: Write `state.rs`** — `AppState { pg: PgPool, app_store: Arc<dyn SqliteAppStore>, base_url: Url, csrf_secret: Key }`. `Clone` derived so axum can clone it per request.

**Step 4: Write `router.rs`** — empty `pub fn router(state: AppState) -> Router` returning a `Router::new()` with the AppState attached.

**Step 5: `lib.rs` re-exports.**

**Step 6: Verify** `cargo build -p lovely-web`.

**Step 7: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: deps, AppState, WebError, empty router"
```

---

### Task 3.2: Page shell view (maud) + static assets

**Files:**
- Create: `crates/lovely-web/src/views/{mod,shell}.rs`
- Create: `static/style.css`
- Create: `static/tree.js`
- Create: `static/fonts/Lora-Regular.woff2`, `Lora-Italic.woff2`, `Lora-Bold.woff2`, `Lora-BoldItalic.woff2`
  - **Action:** download from Google Fonts (https://fonts.google.com/specimen/Lora → "Download family") and copy the `.woff2` files into `static/fonts/`. Verify license is OFL (yes, Lora is OFL).

**Step 1: Write `views/shell.rs`:**

```rust
use maud::{html, Markup, DOCTYPE};

pub struct ShellCtx<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub user: Option<&'a CurrentUser>,
}

pub fn shell(ctx: ShellCtx<'_>, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (ctx.title) }
                @if let Some(d) = ctx.description { meta name="description" content=(d); }
                link rel="stylesheet" href="/static/style.css";
                script src="https://unpkg.com/htmx.org@2.0.4" defer {}
                script src="/static/tree.js" defer {}
            }
            body {
                (top_nav(ctx.user))
                main { (body) }
            }
        }
    }
}
```

**Step 2: Write `static/style.css`** with the tokens from design §13:

```css
:root {
  --ink: #000;
  --paper: #fff;
  --muted-ink: #555;
  --soft-border: #e5e5e5;
  --hairline: #f0f0f0;
  --accent: #c026d3;
  --accent-soft: #fbe7fb;
  --selected-outline: 2px solid var(--accent);
  --focus-ring: 0 0 0 3px color-mix(in oklab, var(--accent) 35%, transparent);
  --font-serif: "Lora", Georgia, serif;
  --font-mono: ui-monospace, Menlo, monospace;
  --radius-1: 4px;
  --space-1: 4px;  --space-2: 8px;  --space-3: 12px;
  --space-4: 16px; --space-6: 24px; --space-8: 32px;
}
@font-face { font-family: 'Lora'; font-style: normal; font-weight: 400;
             src: url('/static/fonts/Lora-Regular.woff2') format('woff2'); font-display: swap; }
@font-face { font-family: 'Lora'; font-style: italic; font-weight: 400;
             src: url('/static/fonts/Lora-Italic.woff2') format('woff2'); font-display: swap; }
@font-face { font-family: 'Lora'; font-style: normal; font-weight: 700;
             src: url('/static/fonts/Lora-Bold.woff2') format('woff2'); font-display: swap; }
@font-face { font-family: 'Lora'; font-style: italic; font-weight: 700;
             src: url('/static/fonts/Lora-BoldItalic.woff2') format('woff2'); font-display: swap; }

* { box-sizing: border-box; }
body { margin: 0; font-family: var(--font-serif); color: var(--ink); background: var(--paper);
       line-height: 1.5; accent-color: var(--accent); }
a { color: var(--ink); text-decoration: underline; }
a:hover { color: var(--accent); }
:focus-visible { outline: none; box-shadow: var(--focus-ring); border-radius: var(--radius-1); }
button, input, select, textarea { font: inherit; }
code, pre { font-family: var(--font-mono); }

nav.top-nav { display: flex; gap: var(--space-4); padding: var(--space-4);
              border-bottom: 1px solid var(--soft-border); }
main { padding: var(--space-6); max-width: 70rem; margin: 0 auto; }
.error { color: #b00020; }
```

**Step 3: Write `static/tree.js`:**

```javascript
(function () {
  const KEY_PREFIX = 'lovely:open:';
  document.addEventListener('toggle', function (e) {
    const el = e.target;
    if (!(el instanceof HTMLDetailsElement) || !el.classList.contains('tree-node')) return;
    localStorage.setItem(KEY_PREFIX + el.dataset.uuid, el.open ? 'open' : 'closed');
  }, true);
  document.addEventListener('DOMContentLoaded', function () {
    document.querySelectorAll('details.tree-node[data-uuid]').forEach(function (el) {
      const v = localStorage.getItem(KEY_PREFIX + el.dataset.uuid);
      if (v === 'open') el.open = true;
      if (v === 'closed') el.open = false;
    });
    const csrf = document.cookie.split('; ').find(c => c.startsWith('csrf_token='));
    if (csrf && window.htmx) {
      window.htmx.config.headers = window.htmx.config.headers || {};
      window.htmx.config.headers['X-CSRF-Token'] = csrf.split('=')[1];
    }
  });
  document.addEventListener('lovely:element-deleted', function (e) {
    const id = e.detail && e.detail.uuid;
    if (!id) return;
    document.getElementById('tree-' + id)?.remove();
    document.getElementById('preview-' + id)?.remove();
  });
})();
```

**Step 4: Wire static-file serving** in `router.rs`:

```rust
use tower_http::services::ServeDir;
Router::new()
    .nest_service("/static", ServeDir::new(state.static_dir.clone()))
    .with_state(state)
```

**Step 5: Add `top_nav` and a stub `CurrentUser` type to `views/shell.rs`.**

**Step 6: Failing test** in `crates/lovely-web/tests/static_assets.rs`:

```rust
#[tokio::test]
async fn serves_style_css_with_correct_mime() {
    let app = lovely_test_support::TestApp::start().await.unwrap();
    let r = reqwest::get(format!("{}/static/style.css", app.url)).await.unwrap();
    assert_eq!(r.status(), 200);
    assert!(r.headers()["content-type"].to_str().unwrap().starts_with("text/css"));
    assert!(r.text().await.unwrap().contains("--accent: #c026d3"));
}
```

(`TestApp` will be added in next task — this test will compile-error until then. That's fine; commit and proceed.)

**Step 7: Commit**

```bash
git add crates/lovely-web/ static/
git commit -m "lovely-web: shell view, style.css with Lora + tokens, tree.js"
```

---

### Task 3.3: `TestApp` — boot full server in tests

**Files:**
- Modify: `crates/lovely-test-support/src/lib.rs`

**Step 1: Implement `TestApp::start`:**

```rust
pub struct TestApp {
    pub url: String,
    pub pg: PgPool,
    pub client: reqwest::Client,
    pub data_dir: TempDir,
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

impl TestApp {
    pub async fn start() -> anyhow::Result<Self> {
        let pg_container = PgTestContainer::start().await?;
        let pg = pg_container.fresh_db().await?;
        let data_dir = TempDir::new()?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let url = format!("http://127.0.0.1:{}", addr.port());
        let state = lovely_web::AppState::new_for_test(pg.clone(), data_dir.path().to_path_buf()).await?;
        let app = lovely_web::router(state);
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            axum::serve(listener, app).with_graceful_shutdown(async { let _ = rx.await; }).await.ok();
        });
        let client = reqwest::Client::builder().cookie_store(true).build()?;
        // Wait for /healthz.
        for _ in 0..50 {
            if let Ok(r) = client.get(format!("{}/healthz", url)).send().await {
                if r.status() == 200 { break; }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(Self { url, pg, client, data_dir, _shutdown: tx })
    }
}
```

**Step 2: Add `/healthz` and `/readyz` handlers** in `lovely-web::router`. `/healthz` returns 200 always. `/readyz` runs `SELECT 1` against the pool.

**Step 3: Run** `cargo test -p lovely-web --test static_assets`. Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-test-support/ crates/lovely-web/
git commit -m "test: TestApp harness; /healthz and /readyz handlers"
```

---

### Task 3.4: Sessions middleware + CSRF

**Files:**
- Create: `crates/lovely-web/src/auth/{mod,session,csrf,extractor}.rs`
- Modify: `crates/lovely-web/src/router.rs`

**Step 1: Failing test** in `tests/csrf.rs`:

```rust
#[tokio::test]
async fn post_without_csrf_token_is_rejected() {
    let app = lovely_test_support::TestApp::start().await.unwrap();
    let r = app.client.post(format!("{}/auth/login", app.url))
        .form(&[("username", "x"), ("password", "y")])
        .send().await.unwrap();
    assert_eq!(r.status(), 403);
}
```

**Step 2: Implement `csrf.rs`** with double-submit cookie. On every GET response, set `csrf_token=<random>; SameSite=Lax`. On every non-GET request, require `X-CSRF-Token` header *or* `_csrf` form field equal to the cookie value.

**Step 3: Implement `session.rs`** using `tower-sessions` + `tower-sessions-sqlx-store::PostgresStore`. Cookie name `lovely_session`, `Secure` in prod, `SameSite=Lax`.

**Step 4: Implement `extractor.rs`** with `AuthUser` and `Option<AuthUser>` extractors (axum `FromRequestParts`). `AuthUser` rejects with `WebError::Unauthorized` if no session.

**Step 5: Run** `cargo test -p lovely-web`. Expected: csrf test passes, others still compile.

**Step 6: Add `docs/rust-notes.md` entry**: "Axum extractors & `FromRequestParts` — `lovely-web/src/auth/extractor.rs`. ELI5: declare `AuthUser` as a function parameter, axum runs your extractor before the handler. If extraction fails (`WebError::Unauthorized`), the handler is never called and the error response is returned. Composes naturally — handlers are 'pure functions' over already-validated input."

**Step 7: Commit**

```bash
git add crates/lovely-web/ docs/rust-notes.md
git commit -m "lovely-web: sessions, CSRF double-submit, AuthUser extractor"
```

---

### Task 3.5: Username/password registration + login

**Files:**
- Create: `crates/lovely-web/src/handlers/{auth_username,mod}.rs`
- Create: `crates/lovely-web/src/views/{auth,mod}.rs`

**Step 1: Failing e2e test** `tests/auth_username.rs`:

```rust
#[tokio::test]
async fn register_and_login_happy_path() {
    let app = lovely_test_support::TestApp::start().await.unwrap();
    // Register
    let token = app.csrf_token().await;
    let r = app.client.post(format!("{}/auth/register", app.url))
        .form(&[("username", "alice"), ("password", "correct horse battery staple"), ("_csrf", &token)])
        .send().await.unwrap();
    assert!(r.status().is_success() || r.status().is_redirection());
    // Logout
    let _ = app.client.post(format!("{}/auth/logout", app.url))
        .form(&[("_csrf", &token)])
        .send().await.unwrap();
    // Login again
    let token = app.csrf_token().await;
    let r = app.client.post(format!("{}/auth/login", app.url))
        .form(&[("username", "alice"), ("password", "correct horse battery staple"), ("_csrf", &token)])
        .send().await.unwrap();
    assert!(r.status().is_success() || r.status().is_redirection());
    // Hit a protected page
    let r = app.client.get(format!("{}/pages", app.url)).send().await.unwrap();
    assert_eq!(r.status(), 200);
}

#[tokio::test]
async fn login_fails_with_wrong_password() { /* ... */ }
#[tokio::test]
async fn anonymous_protected_request_redirects_to_login() { /* assert 302 to /auth/login */ }
#[tokio::test]
async fn anonymous_protected_request_via_htmx_returns_hx_redirect() {
    // Set HX-Request: true; expect 401 + HX-Redirect: /auth/login
}
```

**Step 2: Implement** `views::auth::login_form`, `views::auth::register_form` (maud), `handlers::auth_username::{get_login, post_login, get_register, post_register, post_logout}`. Argon2 hash via `argon2::PasswordHasher`. On success, create session row, set cookie.

**Step 3: Wire into router.**

**Step 4: Run** `cargo test -p lovely-web --test auth_username`. Expected: all pass.

**Step 5: Add `docs/rust-notes.md`**: "Result + `?` + From — `auth_username::post_login`. ELI5: `?` is 'short-circuit on error.' If a function returns `Result<_, DbError>` and you write `db_call().await?`, on `Err` it converts via `From::from` and returns from the enclosing function. The `From` impls in `WebError::from(DbError)` make this seamless."

**Step 6: Commit**

```bash
git add crates/lovely-web/ docs/rust-notes.md
git commit -m "lovely-web: username/password register + login + logout"
```

---

### Task 3.6: TOTP enrollment + verification

**Files:**
- Create: `crates/lovely-web/src/handlers/auth_totp.rs`
- Create: `crates/lovely-web/src/views/totp.rs`

**Step 1: Failing tests** `tests/totp.rs`:

- `enroll_totp_returns_qr_and_secret`
- `login_with_totp_required_redirects_to_verify`
- `verify_totp_with_valid_code_completes_session`
- `verify_totp_with_invalid_code_fails`

**Step 2: Implement** `get_enroll`, `post_enroll`, `post_verify`. Use `totp-rs::TOTP::new` and `qrcode::QrCode` to render the data URL. The "pending login" state between password-success and TOTP-verify is a short-lived signed cookie carrying the user_id.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: TOTP enroll + verify with QR data URL"
```

---

### Task 3.7: OAuth — `AuthProvider` trait + GitHub impl + MockOAuth

**Files:**
- Create: `crates/lovely-web/src/auth/provider.rs`
- Create: `crates/lovely-web/src/auth/oauth_github.rs`
- Create: `crates/lovely-web/src/auth/oauth_mock.rs` (test-only, `#[cfg(test)]` or feature)
- Create: `crates/lovely-web/src/handlers/auth_oauth.rs`

**Step 1: Failing tests** `tests/oauth.rs`:

```rust
#[tokio::test]
async fn oauth_mock_flow_creates_user_and_session() {
    // TestApp::with_mock_oauth("github", deterministic_profile("alice", "alice@example.com"))
    let app = TestApp::start_with_mock_oauth().await.unwrap();
    let r = app.client.get(format!("{}/auth/github", app.url)).send().await.unwrap();
    // Mock provider auto-redirects with a fake code.
    let r = app.client.get(format!("{}/auth/github/callback?code=mock&state={}", app.url, /* state */)).send().await.unwrap();
    assert!(r.status().is_redirection() || r.status() == 200);
    let me = app.client.get(format!("{}/", app.url)).send().await.unwrap().text().await.unwrap();
    assert!(me.contains("alice"));
}
```

**Step 2: Implement `AuthProvider` trait:**

```rust
#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn authorize_url(&self, state: &str, pkce_challenge: &str) -> Url;
    async fn exchange_code(&self, code: &str, pkce_verifier: &str) -> Result<OAuthProfile, WebError>;
}

pub struct OAuthProfile {
    pub provider_user_id: String,
    pub username: Option<String>,
    pub email: Option<String>,
    pub raw: serde_json::Value,
}
```

**Step 3: Implement `GitHubProvider`** using `oauth2` crate. Authorize URL → `https://github.com/login/oauth/authorize`, token URL → `https://github.com/login/oauth/access_token`, profile fetch via `reqwest` against `https://api.github.com/user`.

**Step 4: Implement `MockOAuthProvider`** (deterministic, test-only).

**Step 5: Wire** `/auth/{provider}` and `/auth/{provider}/callback` to a single set of handlers parameterized by provider name. State token stored in a short-lived signed cookie.

**Step 6: Run** the OAuth e2e test. Expected: pass.

**Step 7: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: OAuth AuthProvider trait, GitHub impl, MockOAuth"
```

---

### Task 3.8: Google OAuth provider

**Files:**
- Create: `crates/lovely-web/src/auth/oauth_google.rs`

**Step 1: Failing test** — same shape as GitHub but `name() = "google"`, real authorize URL `https://accounts.google.com/o/oauth2/v2/auth`, profile URL `https://openidconnect.googleapis.com/v1/userinfo`.

**Step 2: Implement.** Mostly copy-paste-edit from GitHub provider; both are vanilla OAuth2 + JSON profile fetch.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: Google OAuth provider"
```

---

### Task 3.9: Apple Sign In provider

**Files:**
- Create: `crates/lovely-web/src/auth/oauth_apple.rs`

**Step 1: Failing test** that uses MockOAuth provider configured to mimic Apple's `form_post` callback (POST not GET).

**Step 2: Implement.** Apple needs:
- Client secret as a JWT signed with the `.p8` private key (ES256). Cache the JWT for 5 minutes. Use `jsonwebtoken` crate.
- Authorize URL `https://appleid.apple.com/auth/authorize` with `response_mode=form_post`.
- Callback handler accepts `POST` form data (in addition to GET).
- Profile fields come from the `id_token` JWT (decode unverified — Apple's signing keys would need separate JWKS fetch; v1 trusts the token came from Apple via the back-channel, since we're exchanging on a TLS connection to Apple's token endpoint).

**Step 3: Add config plumbing** for `--apple-team-id`, `--apple-key-id`, `--apple-services-id`, `--apple-private-key-path`. If any of those is missing at startup, log a warn and skip mounting `/auth/apple` routes (graceful degradation in dev).

**Step 4: Run.** Expected: pass.

**Step 5: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: Apple Sign In with .p8-signed JWT client_secret"
```

---

### Task 3.10: i18n `t!` macro stub

**Files:**
- Create: `crates/lovely-web/src/i18n.rs`

**Step 1: Implement** the simplest possible macro: `t!("login.title")` → `"login.title"` literal for now. Keep it as a single-arg `macro_rules!` so future swap to a real lookup is mechanical.

**Step 2: Replace** every user-facing string literal in `views/auth.rs` and `views/shell.rs` with `t!("...")` calls.

**Step 3: Verify** `cargo test --workspace` still passes.

**Step 4: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: t! macro stub for future i18n"
```

---

## Phase 4 — Pages CRUD

### Task 4.1: List + show page handlers

**Files:**
- Create: `crates/lovely-web/src/handlers/pages.rs`
- Create: `crates/lovely-web/src/views/pages.rs`

**Step 1: Failing tests** `tests/pages.rs`:

- `anonymous_can_view_published_page_by_slug`
- `unpublished_page_404s_to_anonymous`
- `pages_index_requires_auth_and_lists_users_pages`
- `pages_index_renders_no_pages_message_when_empty`
- `published_page_renders_elements_in_order`

**Step 2: Implement** `get_pages_index`, `get_page_by_slug`. The latter loads `pages` row, then `load_elements_for_page`, then `Tree::from_db_rows`, then `tree.render()`.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: pages list + public render via Tree::render"
```

---

### Task 4.2: Create page (form + handler)

**Files:**
- Modify: `crates/lovely-web/src/handlers/pages.rs`
- Modify: `crates/lovely-web/src/views/pages.rs`

**Step 1: Failing tests:**

- `create_page_with_valid_slug_persists_and_redirects`
- `create_page_with_duplicate_slug_renders_form_error`
- `create_page_form_requires_auth`
- `create_page_form_submits_via_htmx_returns_fragment`
- `create_page_form_submits_normally_returns_redirect`

**Step 2: Implement** `get_pages_new`, `post_pages_create`. Validates slug (`^[a-z0-9-]{1,80}$`), normalizes title/description. Creates page + root element (a `Div`) inside one transaction.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: create page via form (htmx-aware)"
```

---

### Task 4.3: Edit metadata + delete page

**Files:**
- Modify: `crates/lovely-web/src/handlers/pages.rs`

**Step 1: Failing tests:**

- `update_page_metadata_persists`
- `update_page_metadata_requires_owner`
- `delete_page_cascades_elements`
- `delete_page_returns_204_for_htmx`

**Step 2: Implement** `post_pages_update`, `delete_pages_delete`.

**Step 3: Run.** Expected: pass.

**Step 4: Commit**

```bash
git add crates/lovely-web/
git commit -m "lovely-web: edit metadata + delete page"
```

---

## Phase 5 — Server binary, config, deploy

### Task 5.1: `lovely-server` binary with `clap` config

**Files:**
- Modify: `crates/lovely-server/Cargo.toml`
- Modify: `crates/lovely-server/src/main.rs`
- Create: `.env.example`

**Step 1: Cargo deps:** `tokio`, `clap`, `dotenvy`, `tracing-subscriber`, `anyhow`, `secrecy`, `lovely-db`, `lovely-web`.

**Step 2: Implement** `main.rs`:

```rust
use clap::Parser;
use secrecy::{Secret, ExposeSecret};

#[derive(Parser, Debug)]
#[command(name = "lovely-server", version)]
struct Args {
    #[arg(long, env = "LOVELY_BIND", default_value = "0.0.0.0:8080")]
    bind: String,
    #[arg(long, env = "LOVELY_DATABASE_URL")]
    database_url: String,
    #[arg(long, env = "LOVELY_SQLITE_DATA_DIR", default_value = "./data/apps")]
    sqlite_data_dir: std::path::PathBuf,
    #[arg(long, env = "LOVELY_BASE_URL", default_value = "http://localhost:8080")]
    base_url: String,
    #[arg(env = "LOVELY_SESSION_SECRET")]
    session_secret: Secret<String>,
    #[arg(long, env = "LOVELY_GITHUB_CLIENT_ID")]
    github_client_id: Option<String>,
    #[arg(env = "LOVELY_GITHUB_CLIENT_SECRET")]
    github_client_secret: Option<Secret<String>>,
    // ... google + apple analogues
    #[arg(long, env = "LOVELY_LOG_FORMAT", default_value = "auto")]
    log_format: String,
    #[arg(long, env = "LOVELY_LOG_LEVEL", default_value = "info")]
    log_level: String,
    #[arg(long, env = "LOVELY_STATIC_DIR", default_value = "./static")]
    static_dir: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("LOVELY_DOTENV").as_deref() == Ok("1") { dotenvy::dotenv().ok(); }
    let args = Args::parse();
    setup_tracing(&args.log_format, &args.log_level)?;
    let pg = lovely_db::pg::connect(&args.database_url).await?;
    let app_store = std::sync::Arc::new(lovely_db::StubSqliteAppStore);
    let state = lovely_web::AppState::new(pg, app_store, args.into()).await?;
    let app = lovely_web::router(state);
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    tracing::info!(addr = %args.bind, "lovely-server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM");
    let mut int  = signal(SignalKind::interrupt()).expect("install SIGINT");
    tokio::select! { _ = term.recv() => {}, _ = int.recv() => {} }
    tracing::info!("shutdown signal received");
}
```

**Step 3: Write `.env.example`** with all the keys, no real secrets.

**Step 4: Verify** `cargo run -p lovely-server --help` prints all flags.

**Step 5: Commit**

```bash
git add crates/lovely-server/ .env.example
git commit -m "lovely-server: clap-based config, dotenv opt-in, graceful shutdown"
```

---

### Task 5.2: `lovely-data` stub binary

**Files:**
- Modify: `crates/lovely-data/src/main.rs`

```rust
fn main() {
    eprintln!("lovely-data: not yet implemented (milestone C+).");
    eprintln!("This binary is reserved for the future remote SQLite app store.");
    std::process::exit(2);
}
```

**Commit:**

```bash
git add crates/lovely-data/
git commit -m "lovely-data: explicit stub for future remote SQLite split"
```

---

### Task 5.3: Multi-stage Dockerfile

**Files:**
- Create: `deploy/Dockerfile`
- Create: `deploy/.dockerignore`

**Step 1: Write Dockerfile** exactly per design §9.

**Step 2: Write `.dockerignore`** excluding `target/`, `.git/`, `tests/`, `docs/`, `data/`, `.env*`.

**Step 3: Verify** `docker build -t lovely-rs:latest -f deploy/Dockerfile .`. Expected: success, image ~30–50MB.

**Step 4: Commit**

```bash
git add deploy/
git commit -m "deploy: multi-stage Dockerfile (rust:1.83 builder + distroless runtime)"
```

---

### Task 5.4: docker-compose for local + Swarm

**Files:**
- Create: `deploy/compose.yaml`

**Step 1: Write** exactly per design §9.

**Step 2: Verify** `docker compose -f deploy/compose.yaml up --build` brings up Postgres + lovely-server, both healthy. Curl `http://localhost:8080/healthz` → 200.

**Step 3: Commit**

```bash
git add deploy/
git commit -m "deploy: compose.yaml for local dev and Swarm"
```

---

### Task 5.5: Kubernetes manifests

**Files:**
- Create: `deploy/k8s/{deployment,service,ingress,pvc,postgres,secret}.yaml`

**Step 1: Write each manifest** per design §9 (replicas:1, strategy: Recreate, RWO PVC, ClusterIP svc, Ingress with cert-manager annotation, StatefulSet for Postgres or external-managed comment).

**Step 2: Verify** `kubectl apply --dry-run=client -f deploy/k8s/`. Expected: all valid.

**Step 3: Commit**

```bash
git add deploy/k8s/
git commit -m "deploy: k8s manifests (Deployment, Service, Ingress, PVCs, Postgres)"
```

---

### Task 5.6: `deploy/README.md`

**Files:**
- Create: `deploy/README.md`

**Step 1: Write** sections: env-var matrix, secrets layout (`docker secret create`, `kubectl create secret`), Postgres major-version upgrade procedure (`pg_upgrade` warning), Apple `.p8` rotation procedure (note the 2026-11-25 calendar reminder from design §12), backup story (documented but not implemented).

**Step 2: Commit**

```bash
git add deploy/
git commit -m "deploy: README covering env, secrets, PG upgrades, Apple key rotation"
```

---

## Phase 6 — Polish + finalize milestone A

### Task 6.1: Root `README.md` and per-crate READMEs

**Files:**
- Create: `README.md`
- Create: `crates/{lovely-tree,lovely-db,lovely-web,lovely-server,lovely-data,lovely-test-support}/README.md`

**Step 1: Root README** — what it is, design link, "how to run locally" (3 commands), "how to test" (1 command), license placeholder.

**Step 2: Per-crate** — one paragraph each: what's in here, what depends on it.

**Step 3: Commit**

```bash
git add README.md crates/
git commit -m "docs: root and per-crate READMEs"
```

---

### Task 6.2: GitHub Actions CI working

**Files:**
- Modify: `.github/workflows/ci.yml`

**Step 1: Update jobs:**

- `fmt`: `cargo fmt --all -- --check`
- `clippy`: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `test`: `cargo test --workspace --all-features` with `services: postgres:17`, set `DATABASE_URL` env var.
- `bench`: only on `push` to `main`; runs `cargo bench --workspace -- --save-baseline ci`.
- `docker`: `docker build -f deploy/Dockerfile .` to verify the image still builds.

**Step 2: Commit**

```bash
git add .github/
git commit -m "ci: full pipeline (fmt, clippy, test w/ Postgres service, bench, docker)"
```

---

### Task 6.3: `cargo sqlx prepare` and commit `.sqlx/`

**Files:**
- Run: `cargo sqlx prepare --workspace`
- Commit: `.sqlx/` directory

**Step 1: Boot Postgres, apply migrations:**

```bash
docker run --rm -d --name lovely-pg -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:17
sleep 3
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres sqlx migrate run --source migrations/
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo sqlx prepare --workspace -- --tests
docker stop lovely-pg
```

**Step 2: Verify** `cargo build --workspace --offline` succeeds.

**Step 3: Update CI** to use `SQLX_OFFLINE=true` so the build job doesn't need a live DB.

**Step 4: Update `.gitignore`** to NOT ignore `.sqlx/` (remove that line).

**Step 5: Commit**

```bash
git add .sqlx/ .gitignore .github/
git commit -m "sqlx: prepare offline metadata; CI builds without live DB"
```

---

### Task 6.4: Smoke test the full deployment locally

**Files:** none — verification only.

**Step 1:**
```bash
docker compose -f deploy/compose.yaml up --build -d
# Wait for healthy
curl -f http://localhost:8080/healthz
curl -f http://localhost:8080/readyz
# Register a user via the form (or via TestApp helpers in a script)
# Visit /pages, /auth/login, etc.
docker compose -f deploy/compose.yaml down -v
```

**Step 2:** Hand-walk the milestone A acceptance checklist (below). Fix any broken bits.

**Step 3: Commit any fixes** with appropriate messages.

---

### Task 6.5: Tag milestone A

**Files:** none.

**Step 1:**
```bash
git tag -a milestone-a -m "Milestone A: Static CMS slice complete"
```

(Don't push yet — user can push when ready.)

---

## Milestone A acceptance checklist (manual)

Each of these must be verifiable end-to-end before declaring milestone A done:

- [ ] `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --all-features` all green
- [ ] `cargo bench -p lovely-tree --features render` produces baseline; six benches measured
- [ ] `cargo doc --workspace --no-deps` produces clean docs with no warnings
- [ ] `docker compose -f deploy/compose.yaml up --build` brings up Postgres + lovely-server, both healthy
- [ ] `kubectl apply --dry-run=client -f deploy/k8s/` validates
- [ ] Register a user via the form, get a session cookie
- [ ] Enroll TOTP, log out, log back in, verify TOTP — succeeds
- [ ] OAuth flow against MockOAuthProvider — creates user + session
- [ ] Create a page with slug, view it as anonymous — renders elements
- [ ] Edit page metadata via htmx — only the targeted fragment swaps
- [ ] Delete a page — gone from `/pages`, 404 at `/pages/:slug`
- [ ] Anonymous request to `/pages` redirects to `/auth/login` (or `HX-Redirect` for htmx)
- [ ] CSRF-less POST to `/auth/login` returns 403
- [ ] Sigterm to `lovely-server` drains and exits 0 within 25s
- [ ] `docs/rust-notes.md` has at least 14 entries (covering the design §10c list)
- [ ] No `// TODO` comments without a name+date
- [ ] No file-header banner comments
- [ ] All `pub` items in `lovely-tree` and `lovely-db` have docstrings

---

## What's next (milestone B preview, not part of this plan)

- `apps` + `app_members` tables
- Build page (3-column grid: tree sidebar, preview, attributes form)
- Targeted htmx OOB swaps for tree mutations
- `user_ui_state` table for last-selected/last-open
- `fantoccini` browser tests
- `define_tags!` typed payloads (Form, Input, etc.)

A separate `docs/plans/<date>-milestone-b-build-page.md` will be written before any milestone B code is touched.

---

*End of milestone A plan.*
