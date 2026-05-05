use crate::views::apps::{app_subnav, AppTab};
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
        (app_subnav(app, AppTab::Data))
        section .summary-section {
            div .section-header {
                h2 { "Collections" }
                a href={"/apps/" (app.slug) "/data/new"} .button { "New collection" }
            }
            @if collections.is_empty() {
                p .muted { "No collections yet." }
            } @else {
                ul .page-list {
                    @for c in collections {
                        li {
                            a href={"/apps/" (app.slug) "/data/" (c.name)} { code { (c.name) } }
                            " "
                            @let fs = c.fields();
                            @if fs.is_empty() {
                                span .muted { "(no fields yet)" }
                            } @else {
                                span .muted { "(" (fs.join(", ")) ")" }
                            }
                        }
                    }
                }
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

pub fn collection_new(
    user: &User,
    app: &App,
    csrf_token: &str,
    error: Option<&str>,
) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / New collection"
        }
        (app_subnav(app, AppTab::Data))
        h1 { "New collection" }
        p .muted { "Create the collection first, then configure its fields." }
        form method="post" action={"/apps/" (app.slug) "/data"} .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Name"
                input type="text" name="name" required pattern="[a-z0-9_]+" maxlength="40"
                      placeholder="posts";
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Create collection" }
        }
    };
    shell(
        ShellCtx {
            title: "New collection",
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
        (app_subnav(app, AppTab::Data))
        div .page-header {
            h1 { (coll.name) }
            div .header-actions {
                a href={"/apps/" (app.slug) "/data/" (coll.name) "/edit"} .button { "Edit fields" }
                form method="post" action={"/apps/" (app.slug) "/data/" (coll.name) "/delete"}
                     .delete-form
                     onsubmit="return confirm('Delete this collection and all its records?')" {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    button type="submit" .danger { "Delete collection" }
                }
            }
        }
        @if fields.is_empty() {
            p .muted {
                "This collection has no fields yet. "
                a href={"/apps/" (app.slug) "/data/" (coll.name) "/edit"} { "Add fields" }
                " to start storing records."
            }
        } @else {
            p .muted { "Fields: " (fields.join(", ")) }
        }

        section .summary-section {
            div .section-header { h2 { "Records" } }
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

        @if !fields.is_empty() {
            section .summary-section {
                div .section-header { h2 { "New record" } }
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

/// Field editor — name + (per-field) rename/delete forms + add-field
/// form. The fields list is the source of truth for which keys appear
/// in record data_json; rename and delete migrate existing records.
pub fn collection_edit(
    user: &User,
    app: &App,
    coll: &Collection,
    csrf_token: &str,
) -> Markup {
    let fields = coll.fields();
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / "
            a href={"/apps/" (app.slug) "/data/" (coll.name)} { code { (coll.name) } }
            " / Edit"
        }
        (app_subnav(app, AppTab::Data))
        h1 { "Edit fields — " (coll.name) }

        section .summary-section {
            div .section-header { h2 { "Fields" } }
            @if fields.is_empty() {
                p .muted { "No fields yet. Add the first one below." }
            } @else {
                ul .field-list {
                    @for f in &fields {
                        li {
                            form method="post"
                                 action={"/apps/" (app.slug) "/data/" (coll.name) "/fields/" (f) "/rename"}
                                 .inline-form {
                                input type="hidden" name="_csrf" value=(csrf_token);
                                input type="text" name="new_name" value=(f) required
                                      pattern="[a-z0-9_]+" maxlength="40";
                                button type="submit" { "Rename" }
                            }
                            form method="post"
                                 action={"/apps/" (app.slug) "/data/" (coll.name) "/fields/" (f) "/delete"}
                                 .inline-form
                                 onsubmit="return confirm('Delete this field? Existing records lose this value.')" {
                                input type="hidden" name="_csrf" value=(csrf_token);
                                button type="submit" .danger { "Delete" }
                            }
                        }
                    }
                }
            }
        }

        section .summary-section {
            div .section-header { h2 { "Add field" } }
            form method="post"
                 action={"/apps/" (app.slug) "/data/" (coll.name) "/fields"}
                 .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                label {
                    "Name"
                    input type="text" name="name" required pattern="[a-z0-9_]+" maxlength="40"
                          placeholder="title";
                }
                button type="submit" { "Add field" }
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("Edit fields — {}", coll.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}
