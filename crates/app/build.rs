use std::{env, fs, path::{Path, PathBuf}};

fn read(path: &Path) -> Option<String> { fs::read_to_string(path).ok().map(|s| s.trim().to_string()) }
fn watch(path: &Path) {
    let mut existing = path;
    while !existing.exists() { let Some(parent) = existing.parent() else { break }; existing = parent; }
    println!("cargo:rerun-if-changed={}", existing.display());
}
fn sha(value: &str) -> Option<String> {
    ((value.len() == 40 || value.len() == 64) && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| value[..12].to_ascii_lowercase())
}
fn checkout_hash(root: &Path) -> Option<String> {
    let dot = root.join(".git"); watch(&dot);
    let git = if dot.is_dir() { dot } else { root.join(read(&dot)?.strip_prefix("gitdir: ")?) };
    let common_file = git.join("commondir"); watch(&common_file);
    let common = read(&common_file).map_or_else(|| git.clone(), |s| git.join(s));
    let head = git.join("HEAD"); watch(&head);
    let head = read(&head)?;
    if let Some(hash) = sha(&head) { return Some(hash) }
    let reference = head.strip_prefix("ref: ")?;
    let loose = common.join(reference); watch(&loose);
    let packed = common.join("packed-refs"); watch(&packed);
    read(&loose).and_then(|s| sha(&s)).or_else(|| {
        read(&packed)?.lines().find_map(|line| {
            let (hash, name) = line.split_once(' ')?;
            (name == reference).then(|| sha(hash)).flatten()
        })
    })
}
fn main() {
    println!("cargo:rerun-if-env-changed=MT_BUILD_HASH");
    println!("cargo:rerun-if-env-changed=MT_MILESTONE");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().and_then(|p| p.parent()).unwrap_or(&manifest);
    let checkout = checkout_hash(root);
    let hash = env::var("MT_BUILD_HASH").ok().filter(|s| !s.trim().is_empty()).map(|s| s.trim().to_string())
        .or(checkout).unwrap_or_else(|| "source archive".into());
    println!("cargo:rustc-env=MT_BUILD_HASH={hash}");
}
