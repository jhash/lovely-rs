use crate::views::{shell, ShellCtx};
use lovely_db::{App, Page, User};
use lovely_tree::ElementTag;
use maud::{html, Markup, PreEscaped};

pub fn pages_new(user: &User, app: &App, csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / New page"
        }
        h1 { "New page in " (app.name) }
        form method="post" action={"/apps/" (app.slug) "/pages"} .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Slug (URL segment, leave empty for the home page)"
                input type="text" name="slug" pattern="[a-z0-9-]*" maxlength="80"
                      placeholder="about-us";
            }
            label {
                "Title"
                input type="text" name="title" required maxlength="200";
            }
            label {
                "Description (optional)"
                textarea name="description" rows="3" maxlength="500" {}
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Create" }
        }
    };
    shell(
        ShellCtx {
            title: "New page",
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

pub fn page_edit(
    user: &User,
    app: &App,
    page: &Page,
    elements: &[lovely_tree::ElementRow],
    preview: Markup,
    csrf_token: &str,
) -> Markup {
    let edit_segment = if page.slug.is_empty() {
        "~home".to_string()
    } else {
        page.slug.clone()
    };
    let public_path = if page.slug.is_empty() {
        format!("/{}", user.username)
    } else {
        format!("/{}/{}", user.username, page.slug)
    };
    let body = html! {
        nav .breadcrumbs {
            a href="/apps" { "Apps" } " / "
            a href={"/apps/" (app.slug)} { (app.name) } " / "
            (if page.slug.is_empty() { "(home)".to_string() } else { page.slug.clone() })
        }
        div .edit-grid {
            section .edit-meta {
                h1 { "Edit: " (page.title) }
                form method="post" action={"/apps/" (app.slug) "/pages/" (edit_segment)} .auth-form {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    label {
                        "Title"
                        input type="text" name="title" value=(page.title) required;
                    }
                    label {
                        "Description"
                        textarea name="description" rows="2" {
                            (page.description.clone().unwrap_or_default())
                        }
                    }
                    label .checkbox {
                        input type="checkbox" name="publish" value="on" checked[page.published_at.is_some()];
                        " Published (visible at "
                        a href=(public_path) { code { (public_path) } }
                        ")"
                    }
                    button type="submit" { "Save" }
                }
                form method="post"
                     action={"/apps/" (app.slug) "/pages/" (edit_segment) "/delete"}
                     .delete-form
                     onsubmit="return confirm('Delete this page?')" {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    button type="submit" .danger { "Delete page" }
                }
            }
            section .edit-elements {
                h2 { "Elements" }
                ul .element-list {
                    @for row in elements {
                        @let is_root = page.root_element == Some(row.id.into_inner());
                        li {
                            details open[is_root] {
                                summary {
                                    code { (row.tag) }
                                    @if is_root { " " span .pill { "root" } }
                                    @if let Some(t) = &row.text {
                                        " — "
                                        span .muted { (t.chars().take(40).collect::<String>()) }
                                    }
                                }
                                form method="post"
                                     action={"/apps/" (app.slug) "/pages/" (edit_segment) "/elements/" (row.id) }
                                     .inline-form {
                                    input type="hidden" name="_csrf" value=(csrf_token);
                                    label {
                                        "Text content"
                                        input type="text" name="text"
                                              value=(row.text.clone().unwrap_or_default());
                                    }
                                    button type="submit" { "Save" }
                                }
                                @if !is_root {
                                    form method="post"
                                         action={"/apps/" (app.slug) "/pages/" (edit_segment) "/elements/" (row.id) "/delete"}
                                         .inline-form
                                         onsubmit="return confirm('Delete this element?')" {
                                        input type="hidden" name="_csrf" value=(csrf_token);
                                        button type="submit" .danger { "Delete" }
                                    }
                                }
                            }
                        }
                    }
                }
                h3 { "Add element" }
                form method="post" action={"/apps/" (app.slug) "/pages/" (edit_segment) "/elements"} .auth-form {
                    input type="hidden" name="_csrf" value=(csrf_token);
                    label {
                        "Tag"
                        select name="tag" required {
                            @for tag in ElementTag::ALL {
                                option value=(tag.name()) { (tag.name()) }
                            }
                        }
                    }
                    label {
                        "Text content (optional)"
                        input type="text" name="text";
                    }
                    button type="submit" { "Add" }
                }
            }
            section .edit-preview {
                h2 { "Preview" }
                div .preview-frame {
                    (preview)
                }
            }
        }
    };
    shell(
        ShellCtx {
            title: &format!("Edit: {}", page.title),
            description: None,
            user: Some(user),
            csrf_token,
        },
        body,
    )
}

pub fn published_page(
    viewer: Option<&User>,
    page: &Page,
    rendered_tree: Markup,
    csrf_token: &str,
) -> Markup {
    let body = html! {
        article .published-page {
            h1 { (page.title) }
            @if let Some(d) = &page.description { p .lead { (d) } }
            (PreEscaped(rendered_tree.into_string()))
        }
    };
    shell(
        ShellCtx {
            title: &page.title,
            description: page.description.as_deref(),
            user: viewer,
            csrf_token,
        },
        body,
    )
}
