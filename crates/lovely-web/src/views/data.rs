use crate::views::{shell, ShellCtx};
use lovely_db::{App, Collection, Record, User};
use maud::{html, Markup};

pub fn data_index(
    user: &User,
    app: &App,
    collections: &[Collection],
    csrf_token: &str,
) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / Data"
        }
        h1 { "Data — " (app.name) }
        section {
            h2 { "Collections" }
            @if collections.is_empty() {
                p .muted { "No collections yet." }
            } @else {
                ul .collection-list {
                    @for c in collections {
                        li {
                            a href={"/apps/" (app.slug) "/data/" (c.name)} { code { (c.name) } }
                            " "
                            span .muted { "(" (c.fields().join(", ")) ")" }
                        }
                    }
                }
            }
        }
        section {
            h2 { "New collection" }
            form method="post" action={"/apps/" (app.slug) "/data"} .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                label {
                    "Name"
                    input type="text" name="name" required pattern="[a-z0-9_]+" maxlength="40"
                          placeholder="posts";
                }
                label {
                    "Fields (comma-separated)"
                    input type="text" name="fields" required placeholder="title, body, slug";
                }
                button type="submit" { "Create collection" }
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("Data — {}", app.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

pub fn collection_view(
    user: &User,
    app: &App,
    coll: &Collection,
    records: &[Record],
    csrf_token: &str,
) -> Markup {
    let fields = coll.fields();
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / "
            code { (coll.name) }
        }
        div .page-header {
            h1 { (coll.name) }
            form method="post" action={"/apps/" (app.slug) "/data/" (coll.name) "/delete"}
                 .delete-form
                 onsubmit="return confirm('Delete this collection and all its records?')" {
                input type="hidden" name="_csrf" value=(csrf_token);
                button type="submit" .danger { "Delete collection" }
            }
        }
        p .muted { "Fields: " (fields.join(", ")) }

        section {
            h2 { "Records" }
            @if records.is_empty() {
                p .muted { "No records yet." }
            } @else {
                table {
                    thead {
                        tr {
                            @for f in &fields { th { (f) } }
                            th {}
                        }
                    }
                    tbody {
                        @for r in records {
                            tr {
                                @for f in &fields {
                                    td {
                                        @if let Some(v) = r.data_json.get(f).and_then(|v| v.as_str()) {
                                            (v)
                                        }
                                    }
                                }
                                td {
                                    form method="post"
                                         action={"/apps/" (app.slug) "/data/" (coll.name) "/records/delete"}
                                         .inline-form {
                                        input type="hidden" name="_csrf" value=(csrf_token);
                                        input type="hidden" name="id" value=(r.id);
                                        button type="submit" .danger { "Delete" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        section {
            h2 { "New record" }
            form method="post" action={"/apps/" (app.slug) "/data/" (coll.name) "/records"} .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                @for f in &fields {
                    label {
                        (f)
                        input type="text" name=(f);
                    }
                }
                button type="submit" { "Add record" }
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("{} — Data", coll.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}
