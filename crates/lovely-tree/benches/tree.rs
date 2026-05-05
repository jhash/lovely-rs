use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lovely_tree::*;
use serde_json::json;

fn random_v4() -> ElementUuid {
    ElementUuid::new_v4()
}

/// Build a flat list of `n` rows that forms a balanced-ish tree:
/// root + first 9 children of root + each child gets ~ (n-10)/9 grandchildren in a chain.
fn generate_rows(n: usize) -> Vec<ElementRow> {
    assert!(n >= 1);
    let mut rows = Vec::with_capacity(n);
    let root = random_v4();
    rows.push(ElementRow {
        id: root,
        parent_id: None,
        prev_sibling: None,
        tag: "div".into(),
        attrs_json: json!({}),
        text: None,
    });
    if n == 1 {
        return rows;
    }
    let trunk_count = 9.min(n - 1);
    let mut trunks: Vec<ElementUuid> = Vec::with_capacity(trunk_count);
    let mut prev: Option<ElementUuid> = None;
    for _ in 0..trunk_count {
        let id = random_v4();
        rows.push(ElementRow {
            id,
            parent_id: Some(root),
            prev_sibling: prev,
            tag: "section".into(),
            attrs_json: json!({}),
            text: None,
        });
        prev = Some(id);
        trunks.push(id);
    }
    let remaining = n.saturating_sub(1 + trunk_count);
    for i in 0..remaining {
        let parent = trunks[i % trunks.len()];
        let id = random_v4();
        rows.push(ElementRow {
            id,
            parent_id: Some(parent),
            prev_sibling: None,
            tag: "p".into(),
            attrs_json: json!({}),
            text: Some("hello".into()),
        });
    }
    // Order siblings: rows that share a parent need prev_sibling chains.
    // The grandchildren above all have prev_sibling=None which fails the
    // builder's "two head rows in same group" check. Patch by walking the
    // grandchild rows and chaining them per-parent.
    let mut last_per_parent: std::collections::HashMap<ElementUuid, ElementUuid> =
        std::collections::HashMap::new();
    for row in rows.iter_mut() {
        let p = match row.parent_id {
            Some(p) => p,
            None => continue,
        };
        if row.prev_sibling.is_some() {
            last_per_parent.insert(p, row.id);
            continue;
        }
        if let Some(prev) = last_per_parent.get(&p).copied() {
            row.prev_sibling = Some(prev);
        }
        last_per_parent.insert(p, row.id);
    }
    rows
}

fn build_tree_with_sample(n: usize) -> (Tree, Vec<ElementUuid>) {
    let rows = generate_rows(n);
    let sample: Vec<ElementUuid> = rows
        .iter()
        .step_by((n / 100).max(1))
        .map(|r| r.id)
        .collect();
    let tree = Tree::from_db_rows(&rows).unwrap();
    (tree, sample)
}

fn bench_build_from_rows_1k(c: &mut Criterion) {
    let rows = generate_rows(1_000);
    c.bench_function("build_from_rows_1k", |b| {
        b.iter(|| {
            let _ = Tree::from_db_rows(black_box(&rows)).unwrap();
        })
    });
}

fn bench_build_from_rows_10k(c: &mut Criterion) {
    let rows = generate_rows(10_000);
    c.bench_function("build_from_rows_10k", |b| {
        b.iter(|| {
            let _ = Tree::from_db_rows(black_box(&rows)).unwrap();
        })
    });
}

fn bench_find_by_uuid(c: &mut Criterion) {
    let (tree, sample) = build_tree_with_sample(1_000);
    c.bench_function("find_by_uuid_1k", |b| {
        b.iter(|| {
            for u in &sample {
                black_box(tree.get_by_uuid(*u));
            }
        })
    });
}

fn bench_insert_in_1k(c: &mut Criterion) {
    let rows = generate_rows(1_000);
    c.bench_function("insert_at_root_in_1k", |b| {
        b.iter_batched(
            || Tree::from_db_rows(&rows).unwrap(),
            |mut tree| {
                let root = tree.root();
                let _ = tree.append_child(root, NewNode::new(ElementUuid::new_v4(), ElementTag::P));
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_remove_in_1k(c: &mut Criterion) {
    let rows = generate_rows(1_000);
    c.bench_function("remove_random_subtree_in_1k", |b| {
        b.iter_batched(
            || {
                let tree = Tree::from_db_rows(&rows).unwrap();
                let some_id = tree
                    .descendants(tree.root())
                    .nth(50)
                    .map(|(id, _)| id)
                    .unwrap_or(tree.root());
                (tree, some_id)
            },
            |(mut tree, id)| {
                if id != tree.root() {
                    let _ = tree.remove(id);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_render_full_1k(c: &mut Criterion) {
    let (tree, _) = build_tree_with_sample(1_000);
    c.bench_function("render_full_1k", |b| {
        b.iter(|| {
            let _ = tree.render().into_string();
        })
    });
}

fn bench_render_subtree_depth_10(c: &mut Criterion) {
    let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
    let mut parent = tree.root();
    for _ in 0..10 {
        parent = tree
            .append_child(parent, NewNode::new(ElementUuid::new_v4(), ElementTag::Div))
            .unwrap();
    }
    c.bench_function("render_subtree_depth_10", |b| {
        b.iter(|| {
            let _ = tree.render_subtree(parent).into_string();
        })
    });
}

criterion_group!(
    benches,
    bench_build_from_rows_1k,
    bench_build_from_rows_10k,
    bench_find_by_uuid,
    bench_insert_in_1k,
    bench_remove_in_1k,
    bench_render_full_1k,
    bench_render_subtree_depth_10
);
criterion_main!(benches);
