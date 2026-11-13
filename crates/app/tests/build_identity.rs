use std::{fs, path::{Path, PathBuf}, process::Command};

fn write(path: &Path, text: &str) { fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, text).unwrap(); }
fn build(app: &Path) -> String {build_profile(app,false)}
fn build_profile(app: &Path, release:bool) -> String {
    let mut command=Command::new(env!("CARGO"));
    command.args(["run","--offline","--quiet"]);if release {command.arg("--release");}
    let out=command.arg("--manifest-path")
        .arg(app.join("Cargo.toml")).arg("--target-dir").arg(app.join("target"))
        .env_remove("MT_BUILD_HASH").env_remove("MT_COMMIT").current_dir(app).output().unwrap();
    assert!(out.status.success(),"{}",String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}
fn fixture(label: &str) -> (PathBuf, PathBuf) {
    let n=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root=std::env::temp_dir().join(format!("mtbi-{label}-{}-{n}",std::process::id()));
    let app=root.join("crates/app");
    write(&app.join("Cargo.toml"),"[package]\nname='build-id-probe'\nversion='0.0.0'\nedition='2021'\n[workspace]\n");
    write(&app.join("src/main.rs"),"fn main() { println!(\"{}\", env!(\"MT_BUILD_HASH\")); }\n");
    write(&app.join("build.rs"),include_str!("../build.rs"));
    (root.canonicalize().unwrap(),app.canonicalize().unwrap())
}
#[test]
fn build_id_rebuilds_after_commit_in_worktree_with_packed_ref() {
    let (root,app)=fixture("worktree"); let common=root.join("metadata"); let linked=common.join("worktrees/linked");
    write(&root.join(".git"),"gitdir: metadata/worktrees/linked\n");
    write(&linked.join("commondir"),"../..\n"); write(&linked.join("HEAD"),"ref: refs/heads/linked\n");
    write(&common.join("packed-refs"),"1111111111111111111111111111111111111111 refs/heads/linked\n");
    assert_eq!(build(&app),"111111111111");
    write(&common.join("refs/heads/linked"),"2222222222222222222222222222222222222222\n");
    assert_eq!(build(&app),"222222222222","commit must invalidate the build script");
    write(&linked.join("HEAD"),"3333333333333333333333333333333333333333\n");
    assert_eq!(build(&app),"333333333333","detached HEAD must invalidate the build script");
}
#[test]
fn build_id_rebuilds_after_commit_in_regular_checkout() {
    let (root,app)=fixture("checkout"); let git=root.join(".git");
    write(&git.join("HEAD"),"ref: refs/heads/main\n");
    write(&git.join("refs/heads/main"),"4444444444444444444444444444444444444444\n");
    assert_eq!(build(&app),"444444444444");
    write(&git.join("refs/heads/main"),"5555555555555555555555555555555555555555\n");
    assert_eq!(build(&app),"555555555555");
}

#[test]
fn same_commit_dev_and_release_share_snapshot_build_identity() {
    let (root,app)=fixture("profiles");
    write(&root.join(".git/HEAD"),"6666666666666666666666666666666666666666\n");
    let dev=build_profile(&app,false);let release=build_profile(&app,true);
    assert_eq!(dev,"666666666666");assert_eq!(release,dev,"profile must not invalidate a snapshot from the same commit");
}
