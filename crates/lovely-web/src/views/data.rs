use crate::views::apps::{app_subnav, AppTab};
use crate::views::{shell, ShellCtx};
use lovely_db::intent::Intent;
use lovely_db::{App, Collection, FieldType, Record, User};
use maud::{html, Markup, PreEscaped};

pub fn data_index(
    user: &User,
    app: &App,
    collections: &[Collection],
    history: &[(i64, Intent)],
    csrf_token: &str,
) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            span .current { "Data" }
        }
        (app_subnav(app, AppTab::Data))
        section .summary-section {
            div .section-header {
                h2 { "Collections" }
                span {
                    a href={"/apps/" (app.slug) "/data/console"} .button.muted { "SQL console" }
                    " "
                    a href={"/apps/" (app.slug) "/data/new"} .button { "New collection" }
                }
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
        @if !history.is_empty() {
            section .summary-section {
                div .section-header {
                    h2 { "Schema history" }
                    p .muted { "Every change applied to this app's SQLite database." }
                }
                ol .schema-history reversed {
                    @for (v, i) in history.iter().rev() {
                        li {
                            span .muted { "v" (v) " " }
                            (i.summary())
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

pub fn collection_new(user: &User, app: &App, csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / "
            span .current { "New collection" }
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
    let typed = coll.typed_fields();
    let fields = coll.fields();
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / "
            span .current { code { (coll.name) } }
        }
        (app_subnav(app, AppTab::Data))
        div .page-header {
            h1 { (coll.name) }
            div .header-actions {
                a href={"/apps/" (app.slug) "/data/" (coll.name) "/edit"} .button { "Edit" }
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
                    @for f in &typed {
                        (typed_field_input(f))
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

/// Collection editor — rename collection + manage fields (per-field
/// rename/delete + add-field with a type picker). Fields are typed
/// objects; the schema list drives which keys + input shapes the
/// record form renders.
pub fn collection_edit(user: &User, app: &App, coll: &Collection, csrf_token: &str) -> Markup {
    let typed = coll.typed_fields();
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / "
            a href={"/apps/" (app.slug) "/data/" (coll.name)} { code { (coll.name) } }
            " / "
            span .current { "Edit" }
        }
        (app_subnav(app, AppTab::Data))
        h1 { "Edit — " (coll.name) }

        section .summary-section {
            div .section-header { h2 { "Collection" } }
            form method="post"
                 action={"/apps/" (app.slug) "/data/" (coll.name) "/rename"}
                 .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                label {
                    "Name"
                    input type="text" name="new_name" value=(coll.name) required
                          pattern="[a-z0-9_]+" maxlength="40";
                }
                button type="submit" { "Save" }
            }
        }

        section .summary-section {
            div .section-header { h2 { "Fields" } }
            @if typed.is_empty() {
                p .muted { "No fields yet. Add the first one below." }
            } @else {
                ul .field-list {
                    @for f in &typed {
                        li .field-row {
                            span .field-type-pill { (f.field_type.label()) }
                            form method="post"
                                 action={"/apps/" (app.slug) "/data/" (coll.name) "/fields/" (f.name) "/rename"}
                                 .inline-form {
                                input type="hidden" name="_csrf" value=(csrf_token);
                                input type="text" name="new_name" value=(f.name) required
                                      pattern="[a-z0-9_]+" maxlength="40";
                                button type="submit" { "Rename" }
                            }
                            form method="post"
                                 action={"/apps/" (app.slug) "/data/" (coll.name) "/fields/" (f.name) "/delete"}
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
                label {
                    "Type"
                    select name="type" required {
                        @for t in FieldType::ALL {
                            option value=(t.as_str())
                                selected[*t == FieldType::Text] { (t.label()) }
                        }
                    }
                }
                button type="submit" { "Add field" }
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("Edit — {}", coll.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

fn typed_field_input(f: &lovely_db::Field) -> Markup {
    let name = &f.name;
    match f.field_type {
        FieldType::Text => html! { label { (name) input type="text" name=(name); } },
        FieldType::LongText => html! {
            label { (name) textarea name=(name) rows="4" {} }
        },
        FieldType::Number => html! { label { (name) input type="number" name=(name); } },
        FieldType::Email => html! { label { (name) input type="email" name=(name); } },
        FieldType::Url => html! { label { (name) input type="url" name=(name); } },
        FieldType::Date => html! { label { (name) input type="date" name=(name); } },
        FieldType::DateTime => html! {
            label { (name) input type="datetime-local" name=(name); }
        },
        FieldType::Bool => html! {
            label .checkbox {
                input type="checkbox" name=(name) value="true";
                " " (name)
            }
        },
        FieldType::Address => html! {
            label {
                (PreEscaped(name.as_str()))
                textarea name=(name) rows="3" placeholder="Street\nCity, State ZIP" {}
            }
        },
    }
}

/// Read-only SQL console for the per-app SQLite database. Lets the
/// owner peek at what the intent log has built so far. The handler
/// rejects anything that isn't a SELECT and caps results at 100 rows.
pub fn data_console(
    user: &User,
    app: &App,
    csrf_token: &str,
    sql: Option<&str>,
    result: Option<Result<ConsoleRows, String>>,
) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            a href={"/apps/" (app.slug) "/data"} { "Data" } " / "
            span .current { "Console" }
        }
        (app_subnav(app, AppTab::Data))
        section .summary-section {
            div .section-header {
                h2 { "SQL console" }
                p .muted { "Read-only SELECT against this app's SQLite database." }
            }
            form method="post" action={"/apps/" (app.slug) "/data/console"} .auth-form {
                input type="hidden" name="_csrf" value=(csrf_token);
                label {
                    "Query"
                    textarea name="sql" rows="6" placeholder="SELECT * FROM posts LIMIT 10" autofocus {
                        @if let Some(s) = sql { (s) }
                    }
                }
                button type="submit" { "Run" }
            }
            @match result {
                Some(Ok(rows)) => (console_results(&rows)),
                Some(Err(msg)) => p .error { (msg) },
                None => {}
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("Console — {}", app.name),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

#[derive(Debug, Clone)]
pub struct ConsoleRows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

fn console_results(rows: &ConsoleRows) -> Markup {
    html! {
        @if rows.rows.is_empty() {
            p .muted { "(no rows)" }
        } @else {
            table .data-table {
                thead {
                    tr {
                        @for c in &rows.columns { th { (c) } }
                    }
                }
                tbody {
                    @for r in &rows.rows {
                        tr {
                            @for cell in r { td { (cell) } }
                        }
                    }
                }
            }
            @if rows.truncated {
                p .muted { "Result truncated at 100 rows." }
            }
        }
    }
}
