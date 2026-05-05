# lovely-tree

The Page Element DOM data structure that backs lovely-rs's renderer and live editor.

`Tree` is a slotmap arena with doubly-linked siblings, first/last child pointers, and a `HashMap<ElementUuid, NodeId>` side-table for O(1) lookups. Iterators (`children`, `ancestors`, `descendants`) are lazy and zero-allocation. The `define_tags!` macro is the single source of truth for the HTML tag whitelist — `<script>`, `<iframe>`, `<style>`, etc., are not in it and never will be.

Render is iterative (explicit `Vec` stack), so deeply nested user content can't overflow the call stack. Behind the `render` feature flag.

## Bench it

```sh
cargo bench -p lovely-tree --features render
```

Indicative numbers on an Apple Silicon laptop:
- `render_subtree_depth_10`: ~55 ns
- `render_full_1k`: ~17 µs
- `find_by_uuid_1k`: ~1.0 µs
- `build_from_rows_1k`: ~238 µs

## What depends on this

`lovely-db` (loads rows into a Tree), `lovely-web` (renders pages).
