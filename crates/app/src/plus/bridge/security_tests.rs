use super::*;
use super::tests::{Temp, git, git_fixture};
use std::os::windows::fs::symlink_file;
const ID: &str = "20261028-123456-mt1";

fn file_symlink(target: &Path, link: &Path) -> bool {
    match symlink_file(target, link) {
        Ok(()) => true,
        Err(e) if e.raw_os_error() == Some(1314) => {
            assert!(std::env::var_os("MT_BRIDGE_REQUIRE_SYMLINK").is_none(), "file symlink capability required: {e}");
            eprintln!("CAPABILITY: file symlink fixture unavailable (Windows privilege 1314); hardlink/junction guards are tested separately"); false
        }
        Err(e) => panic!("symlink fixture: {e}"),
    }
}

fn junction(link: &Path, target: &Path) {
    let script = format!("New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null", link.display().to_string().replace('\'', "''"), target.display().to_string().replace('\'', "''"));
    let result = Command::new("pwsh.exe").args(["-NoProfile", "-NonInteractive", "-Command", &script]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}
fn preserve(root: &Path, work: &Path) {
    assert!(work.exists());
    assert!(git(root, &["branch", "--list"]).contains(&format!("mt-bug/{ID}")));
}

#[test]
fn git_environment_and_repository_identity_are_independent_guards() {
    let a = Temp::new(); let (root, work) = git_fixture(&a);
    let b = Temp::new(); let (other, other_work) = git_fixture(&b);
    let mut command = Command::new("git.exe");
    command.current_dir(&root).args(["rev-parse", "--show-toplevel"]);
    for key in ["GIT_DIR", "GIT_COMMON_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_CONFIG", "GIT_CONFIG_COUNT", "GIT_CONFIG_PARAMETERS", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_CEILING_DIRECTORIES"] {
        command.env(key, other.join(".git"));
    }
    process::sanitize_git(&mut command);
    assert!(command.get_envs().all(|(k, v)| !k.to_string_lossy().starts_with("GIT_") || v.is_none()));
    let result = process::output(&mut command, Duration::from_secs(5)).unwrap();
    assert!(result.status.success());
    assert_eq!(Path::new(String::from_utf8_lossy(&result.stdout).trim()).canonicalize().unwrap(), root.canonicalize().unwrap());
    struct Redirect(PathBuf);
    impl Runner for Redirect {
        fn run(&mut self, _tool: Tool, _root: &Path, args: &[OsString]) -> io::Result<String> { System.run(Tool::Git, &self.0, args) }
    }
    assert!(discard(&root, ID, &mut Redirect(other.clone())).unwrap_err().to_string().contains("identity"));
    preserve(&root, &work); preserve(&other, &other_work);
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--exact", "plus::bridge::security_tests::inherited_git_environment_child", "--ignored", "--nocapture"])
        .env("MT_BRIDGE_TEST_ROOT", &root).env("GIT_DIR", other.join(".git"))
        .env("GIT_COMMON_DIR", other.join(".git")).env("GIT_WORK_TREE", &other)
        .env("GIT_INDEX_FILE", other.join(".git/index"))
        .env("GIT_CONFIG_COUNT", "1").env("GIT_CONFIG_KEY_0", "status.showUntrackedFiles").env("GIT_CONFIG_VALUE_0", "no");
    let output = process::output(&mut child, Duration::from_secs(10)).unwrap();
    assert!(output.status.success(), "{} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(!work.exists()); preserve(&other, &other_work);
}

#[test]
#[ignore = "subprocess fixture; parent test injects a hostile environment"]
fn inherited_git_environment_child() {
    let root = PathBuf::from(std::env::var_os("MT_BRIDGE_TEST_ROOT").expect("parent fixture root"));
    discard(&root, ID, &mut System).unwrap();
}

#[test]
fn hidden_nested_untracked_work_survives_discard() {
    let tmp = Temp::new(); let (root, work) = git_fixture(&tmp);
    git(&root, &["config", "status.showUntrackedFiles", "no"]);
    fs::create_dir_all(work.join("new/deep")).unwrap();
    fs::write(work.join("new/deep/source.rs"), "unfinished").unwrap();
    assert!(git(&work, &["status", "--porcelain"]).is_empty(), "fixture actually hides untracked files");
    assert_eq!(discard(&root, ID, &mut System).unwrap_err().to_string(), "Fix worktree has uncommitted or untracked files");
    assert_eq!(fs::read(work.join("new/deep/source.rs")).unwrap(), b"unfinished"); preserve(&root, &work);
}

#[test]
fn notes_refuse_hardlinks_symlinks_and_open_replacement() {
    let tmp = Temp::new(); let report = tmp.report(); let victim = tmp.0.join("victim");
    fs::write(&victim, "untouched").unwrap(); let path = report.directory.join("notes.jsonl");
    fs::hard_link(&victim, &path).unwrap();
    assert!(note(&tmp.0, &report, "hardlink").is_err());
    assert_eq!(fs::read(&victim).unwrap(), b"untouched"); fs::remove_file(&path).unwrap();
    fs::write(&path, "old").unwrap();
    assert!(files::append(&path, b"injected", || {
        fs::remove_file(&path).unwrap(); fs::hard_link(&victim, &path).unwrap();
    }, || {}).is_err());
    assert_eq!(fs::read(&victim).unwrap(), b"untouched"); fs::remove_file(&path).unwrap();
    fs::write(&path, "old").unwrap();
    files::append(&path, b"new", || {
        assert!(fs::rename(&report.directory, tmp.0.join("before-open-directory")).is_err(), "parents must be pinned before opening notes");
    }, || {
        assert!(fs::rename(&path, tmp.0.join("moved")).is_err(), "opened file must stay pinned");
        assert!(fs::rename(&report.directory, tmp.0.join("moved-directory")).is_err(), "parents must stay pinned");
    }).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"oldnew");
    assert!(files::append(&path, b"injected", || {}, || {
        fs::hard_link(&path, tmp.0.join("new-link")).unwrap();
    }).is_err(), "link count must be checked on the opened handle");
    assert_eq!(fs::read(tmp.0.join("new-link")).unwrap(), b"oldnew"); fs::remove_file(&path).unwrap();
    if !file_symlink(&victim, &path) { return; }
    assert!(note(&tmp.0, &report, "symlink").is_err());
    assert_eq!(fs::read(&victim).unwrap(), b"untouched"); fs::remove_file(&path).unwrap();
    fs::write(&path, "old").unwrap();
    let result = files::append(&path, b"injected", || {
        fs::remove_file(&path).unwrap(); symlink_file(&victim, &path).unwrap();
    }, || {});
    assert!(result.is_err()); assert_eq!(fs::read(&victim).unwrap(), b"untouched");
}

#[test]
fn junction_bundles_and_worktrees_are_refused() {
    let tmp = Temp::new(); let report = tmp.report(); let outside = tmp.0.join("outside");
    fs::rename(&report.directory, &outside).unwrap(); junction(&report.directory, &outside);
    let victim = outside.join("notes.jsonl"); fs::write(&victim, "untouched").unwrap();
    assert!(note(&tmp.0, &report, "bad").unwrap_err().to_string().contains("outside"));
    assert!(files::append(&report.directory.join("notes.jsonl"), b"bad", || {}, || {}).unwrap_err().to_string().contains("Redirected"));
    assert_eq!(fs::read(&victim).unwrap(), b"untouched");
    fs::remove_dir(&report.directory).unwrap();
    let repo = Temp::new(); let (root, work) = git_fixture(&repo);
    let moved = repo.0.join("outside-work"); fs::rename(&work, &moved).unwrap(); junction(&work, &moved);
    assert!(discard(&root, ID, &mut System).unwrap_err().to_string().contains("redirected"));
    assert!(moved.join(".git").exists()); preserve(&root, &work);
    fs::remove_dir(&work).unwrap();
}

#[test]
fn status_rejects_links_nonregular_oversized_malformed_and_nested_json() {
    let tmp = Temp::new(); let report = tmp.report(); let path = report.directory.join("status.json");
    let victim = tmp.0.join("outside-status"); fs::write(&victim, r#"{"state":"received"}"#).unwrap();
    if file_symlink(&victim, &path) {
        assert!(status(&tmp.0, &report).unwrap_err().to_string().contains("regular")); fs::remove_file(&path).unwrap();
    }
    fs::create_dir(&path).unwrap();
    assert!(status(&tmp.0, &report).unwrap_err().to_string().contains("regular")); fs::remove_dir(&path).unwrap();
    fs::write(&path, format!(r#"{{"state":"received","padding":"{}"}}"#, "x".repeat(LIMIT as usize))).unwrap();
    assert!(status(&tmp.0, &report).unwrap_err().to_string().contains("too large"));
    fs::write(&path, b"{broken").unwrap(); assert!(status(&tmp.0, &report).is_err());
    assert_eq!(fs::read(&victim).unwrap(), br#"{"state":"received"}"#);
}

#[test]
fn status_nested_json_is_refused() {
    let tmp = Temp::new(); let report = tmp.report();
    fs::write(report.directory.join("status.json"), format!(r#"{{"state":"received","nested":{}0{}}}"#, "[".repeat(150), "]".repeat(150))).unwrap();
    assert!(status(&tmp.0, &report).unwrap_err().to_string().contains("recursion limit"));
}

#[test]
fn ping_neutralizes_esc_ansi_c0_c1_and_unicode_separators() {
    let text = "\x1b[31mred\x1b[0m\x00\x07\x7f\u{85}\u{9b}\u{2028}\u{2029}";
    let line = ping(ID, &json!({"id":ID,"kind":"bug","text":text})).unwrap();
    assert!(line.ends_with(" ::  [31mred [0m       "), "{line:?}");
    assert!(!line.chars().any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}'));
}

#[test]
fn command_deadline_kills_pipe_holding_descendants_on_exit_timeout_and_error() {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::Threading::*;
    for mode in ["exit", "timeout", "error"] {
        let tmp = Temp::new(); let pidfile = tmp.0.join("pids"); let script = tmp.0.join("fixture.py");
        fs::write(&script, "import os,sys,subprocess,time\np = subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)'], stdin=subprocess.DEVNULL)\nopen(sys.argv[1],'w').write(str(os.getpid())+' '+str(p.pid))\nif sys.argv[2]=='timeout': time.sleep(30)\nif sys.argv[2]=='error': sys.exit(7)\n").unwrap();
        let mut command = Command::new("python.exe"); command.args([script.as_os_str(), pidfile.as_os_str(), OsStr::new(mode)]);
        let start = Instant::now(); let result = process::output(&mut command, Duration::from_millis(600));
        if mode == "error" { assert_eq!(result.unwrap().status.code(), Some(7)); }
        else { assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut); }
        assert!(start.elapsed() < Duration::from_secs(3), "whole command exceeded deadline");
        for pid in fs::read_to_string(&pidfile).unwrap().split_whitespace().map(|s| s.parse::<u32>().unwrap()) {
            let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, 0, pid) };
            if !raw.is_null() {
                let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
                let waited = unsafe { WaitForSingleObject(handle.as_raw_handle(), 1500) };
                if waited != 0 { unsafe { TerminateProcess(handle.as_raw_handle(), 99); WaitForSingleObject(handle.as_raw_handle(), 1500); } }
                assert_eq!(waited, 0, "process {pid} survived {mode}");
            }
        }
    }
}
