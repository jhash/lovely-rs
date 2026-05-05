use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let v = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=ASSET_VERSION={v}");
    println!("cargo:rerun-if-changed=build.rs");
}
