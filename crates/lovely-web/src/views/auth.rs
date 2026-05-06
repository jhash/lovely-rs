use crate::views::{shell, ShellCtx};
use maud::{html, Markup};

pub fn login_page(csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        h1 { "Sign in" }
        form method="post" action="/auth/login" .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Username"
                input type="text" name="username" required autocomplete="username";
            }
            label {
                "Password"
                input type="password" name="password" autocomplete="current-password";
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Sign in" }
        }
        div .oauth-buttons {
            a href="/auth/github" { "Sign in with GitHub" }
            a href="/auth/google" { "Sign in with Google" }
            a href="/auth/apple"  { "Sign in with Apple" }
        }
        p { "No account? " a href="/auth/register" { "Register" } "." }
    };
    shell(
        ShellCtx {
            title: "Sign in",
            description: Some("Sign in to lovely"),
            user: None,
            csrf_token,
        },
        body,
    )
}

pub fn register_page(csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        h1 { "Register" }
        form method="post" action="/auth/register" .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Username"
                input type="text" name="username" required minlength="3" maxlength="40"
                      data-slug-input
                      hx-get="/auth/check-username"
                      hx-trigger="input changed delay:300ms"
                      hx-target="next .slug-feedback"
                      hx-swap="innerHTML";
                span .slug-feedback aria-live="polite" {}
            }
            label {
                "Email (optional)"
                input type="email" name="email" autocomplete="email";
            }
            label {
                "Password"
                input type="password" name="password" required minlength="8";
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Create account" }
        }
        p { "Already have an account? " a href="/auth/login" { "Sign in" } "." }
    };
    shell(
        ShellCtx {
            title: "Register",
            description: Some("Create a lovely account"),
            user: None,
            csrf_token,
        },
        body,
    )
}

pub fn totp_enroll_page(csrf_token: &str, qr_data_url: &str, secret_b32: &str) -> Markup {
    let body = html! {
        h1 { "Enable two-factor authentication" }
        p { "Scan this QR code with an authenticator app (1Password, Authy, Google Authenticator)." }
        img src=(qr_data_url) alt="TOTP QR code" .totp-qr;
        details { summary { "Or enter this secret manually" } code { (secret_b32) } }
        form method="post" action="/auth/totp/enroll" .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "Confirm with the 6-digit code"
                input type="text" name="code" inputmode="numeric" pattern="[0-9]{6}" required;
            }
            button type="submit" { "Enable 2FA" }
        }
    };
    shell(
        ShellCtx {
            title: "Enable 2FA",
            description: None,
            user: None,
            csrf_token,
        },
        body,
    )
}

pub fn totp_verify_page(csrf_token: &str, error: Option<&str>) -> Markup {
    let body = html! {
        h1 { "Two-factor verification" }
        form method="post" action="/auth/totp/verify" .auth-form {
            input type="hidden" name="_csrf" value=(csrf_token);
            label {
                "6-digit code"
                input type="text" name="code" inputmode="numeric" pattern="[0-9]{6}" required autofocus;
            }
            @if let Some(msg) = error { p .error { (msg) } }
            button type="submit" { "Verify" }
        }
    };
    shell(
        ShellCtx {
            title: "Two-factor verification",
            description: None,
            user: None,
            csrf_token,
        },
        body,
    )
}
