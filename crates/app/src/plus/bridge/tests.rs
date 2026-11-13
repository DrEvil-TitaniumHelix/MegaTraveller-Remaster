use super::*;
use std::{collections::VecDeque, sync::atomic::{AtomicU64, Ordering}};
const ID: &str = "20261028-123456-mt1";
static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Temp(pub(super) PathBuf);
impl Temp {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("mt-bridge-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn report(&self) -> Report {
        let directory = self.0.join("acceptance/local/bugs").join(ID); fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("bundle.json"), serde_json::to_vec(&json!({"id":ID,"kind":"bug","text":"a report"})).unwrap()).unwrap();
        Report { id: ID.into(), directory }
    }
}
impl Drop for Temp { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
#[derive(Default)]
struct Fake { calls: Vec<(Tool, Vec<OsString>)>, replies: VecDeque<io::Result<String>> }
impl Runner for Fake {
    fn run(&mut self, tool: Tool, _root: &Path, args: &[OsString]) -> io::Result<String> {
        self.calls.push((tool, args.to_vec())); self.replies.pop_front().unwrap_or(Ok(String::new()))
    }
}
fn strs(args: &[&str]) -> Vec<OsString> { args.iter().map(OsString::from).collect() }
fn atomic_json(path: &Path, value: &Value) -> io::Result<()> { write_json(path, value, true) }
fn fixed() -> Status { Status { state: "fixed".into(), message: "ready to review".into(), commit: Some("abc1234".into()) } }
fn settle(state: &mut State, root: &Path, report: &Report, now: Instant) {
    let until = Instant::now() + Duration::from_secs(5);
    while state.busy() {
        state.poll(root.to_owned(), report, now);
        assert!(Instant::now() < until, "worker did not finish"); thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn ping_uses_saved_kind_one_argument_and_eighty_unicode_characters() {
    let tmp = Temp::new(); let report = tmp.report();
    let text = format!("quotes \" & $(whoami)\r\n{}", "é".repeat(90));
    let bundle = json!({"id":ID,"kind":"qol","text":text});
    fs::write(report.directory.join("bundle.json"), serde_json::to_vec(&bundle).unwrap()).unwrap();
    let mut fake = Fake::default();
    fake.replies.push_back(Ok(json!({"result":{"agents":[{"name":"mtforeman-old","pane_id":"wH:p9"},{"name":"mtforeman","pane_id":"wH:p1"}]}}).to_string()));
    assert!(send(&tmp.0, &report, Some("wH:p2"), &mut fake).unwrap().starts_with("Sent"));
    let line = ping(ID, &bundle).unwrap();
    assert_eq!(line, format!("BUGBRIDGE v1 {ID} qol acceptance/local/bugs/{ID}/bundle.json :: {}", text.replace(['\r', '\n'], " ").chars().take(80).collect::<String>()));
    assert_eq!(fake.calls, [(Tool::Herdr, strs(&["agent", "list"])), (Tool::Herdr, strs(&["pane", "send-text", "wH:p1", &line])), (Tool::Herdr, strs(&["pane", "send-keys", "wH:p1", "enter"]))]);
    assert!(!report.directory.join("status.json").exists());
    assert!(ping(ID, &json!({"id":ID,"kind":";git","text":"x"})).is_err());
    assert!(ping(ID, &json!({"id":"another","kind":"bug","text":"x"})).is_err());
}
#[test]
fn queued_without_pane_fallback_and_send_failure_order() {
    let tmp = Temp::new(); let report = tmp.report(); let mut fake = Fake::default();
    assert_eq!(send(&tmp.0, &report, None, &mut fake).unwrap(), "queued: no foreman pane");
    assert_eq!(fake.calls.len(), 1);
    let s = status(&tmp.0, &report).unwrap().unwrap(); assert_eq!(s.state, "queued"); assert_eq!(s.message, "no foreman pane");
    let v = read_json(&report.directory.join("status.json")).unwrap(); assert_eq!(v["reason"], "no foreman pane");
    fake = Fake::default(); fake.replies.push_back(Err(io::Error::other("not installed")));
    send(&tmp.0, &report, Some("wH:p7"), &mut fake).unwrap(); assert_eq!(fake.calls[1].1[2], "wH:p7");
    fake = Fake::default(); fake.replies.extend([Ok("{}".into()), Err(io::Error::other("send failed"))]);
    assert!(send(&tmp.0, &report, Some("wH:p7"), &mut fake).is_err()); assert_eq!(fake.calls.len(), 2, "no Enter after failed text");
    for p in ["", "--help", "wH:p1\ntext", "wH:p1 ; exit"] { assert!(!pane_ok(p)); }
}
#[test]
fn ids_paths_and_statuses_are_checked_before_side_effects() {
    let tmp = Temp::new(); let mut report = tmp.report();
    for id in [ID, "20261028-123456-mt1-2", "20261028-123456-mt1-10000"] { validate_id(id).unwrap(); }
    for id in ["../main", "20261028-123456-mt1/../main", "20261028-123456-mt1\\x", "20261028-123456-mt1-0", "20261028-123456-mt1-1", "20261028-123456-mt1-02", "20261028-123456-mt1-10001", "20261028-123456-mt1-", "２０２６1028-123456-mt1", "20261028-123456-mt1\n"] {
        let mut fake = Fake::default(); assert!(validate_id(id).is_err(), "{id}");
        assert!(discard(&tmp.0, id, &mut fake).is_err()); assert!(accept(&tmp.0, id, &mut fake).is_err()); assert!(fake.calls.is_empty());
    }
    let other = Temp::new(); report.directory = other.0.clone();
    assert!(send(&tmp.0, &report, None, &mut Fake::default()).is_err());
    let report = tmp.report();
    for value in [json!({"state":"fixed"}), json!({"state":"unknown"}), json!({"id":"wrong","state":"received"}), json!({"state":"fixed","commit":"--delete"})] {
        atomic_json(&report.directory.join("status.json"), &value).unwrap(); assert!(status(&tmp.0, &report).is_err());
    }
}
#[test]
fn round_trip_status_notes_and_five_second_poll() {
    let tmp = Temp::new(); let report = tmp.report();
    let mut fake = Fake::default(); fake.replies.push_back(Ok(json!({"agents":[{"name":"mtforeman","pane_id":"wH:p1"}]}).to_string()));
    send(&tmp.0, &report, None, &mut fake).unwrap();
    let mut state = State::default(); state.saved(&report); let start = Instant::now();
    for (index, remote) in STATES.iter().enumerate() {
        atomic_json(&report.directory.join("status.json"), &json!({"id":ID,"state":remote,"at":123,"lane":"fake-fixer","commit":"abc1234","message":format!("message {index}")})).unwrap();
        let now = start + POLL * index as u32;
        if index > 0 { state.poll(tmp.0.clone(), &report, now - Duration::from_millis(1)); assert!(!state.busy(), "must not read before 5 s"); }
        state.poll(tmp.0.clone(), &report, now); settle(&mut state, &tmp.0, &report, now);
        let s = state.remote.as_ref().unwrap(); assert_eq!(s.state, *remote); assert_eq!(s.message, format!("message {index}"));
    }
    note(&tmp.0, &report, "first\nnote").unwrap(); note(&tmp.0, &report, "second").unwrap();
    let notes = fs::read_to_string(report.directory.join("notes.jsonl")).unwrap();
    let notes: Vec<Value> = notes.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(notes.len(), 2); assert_eq!(notes[0]["text"], "first note"); assert_eq!(notes[1]["text"], "second"); assert!(notes[0]["at"].is_u64());
    assert!(note(&tmp.0, &report, " \n ").is_err());
    let before = fs::read(report.directory.join("status.json")).unwrap();
    write_json(&report.directory.join("status.json"), &json!({"state":"queued"}), false).unwrap();
    assert_eq!(before, fs::read(report.directory.join("status.json")).unwrap(), "atomic publication must not clobber a concurrent status");
    send(&tmp.0, &report, None, &mut Fake::default()).unwrap(); assert_eq!(before, fs::read(report.directory.join("status.json")).unwrap(), "sender preserves fixer's status");
}
pub(super) fn git(root: &Path, args: &[&str]) -> String { run(&mut System, Tool::Git, root, args).unwrap() }
pub(super) fn git_fixture(tmp: &Temp) -> (PathBuf, PathBuf) {
    let root = tmp.0.join("MegaTraveller-Remaster"); fs::create_dir(&root).unwrap();
    git(&root, &["init", "-b", "main"]);
    git(&root, &["-c", "user.name=Bridge Test", "-c", "user.email=bridge@example.invalid", "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "fixture"]);
    let work = tmp.0.join("_lanes").join(format!("MegaTraveller-Remaster-bug-{ID}")); fs::create_dir_all(work.parent().unwrap()).unwrap();
    git(&root, &["worktree", "add", "-b", &format!("mt-bug/{ID}"), work.to_str().unwrap()]);
    (root, work)
}
#[test]
fn accept_keeps_and_discard_removes_exact_worktree_and_branch_in_temp_repo() {
    let tmp = Temp::new(); let (root, work) = git_fixture(&tmp);
    let main_before = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["branch", "keep-me"]);
    assert_eq!(accept(&root, ID, &mut System).unwrap(), format!("git merge mt-bug/{ID}"));
    assert!(work.exists()); assert!(git(&root, &["branch", "--list"]).contains(&format!("mt-bug/{ID}")));
    discard(&root, ID, &mut System).unwrap();
    assert!(!work.exists()); assert!(!git(&root, &["branch", "--list"]).contains(&format!("mt-bug/{ID}")));
    assert!(git(&root, &["branch", "--list"]).contains("keep-me")); assert_eq!(git(&root, &["rev-parse", "HEAD"]), main_before);
}
#[test]
fn discard_preserves_dirty_locked_wrong_branch_and_current_worktrees() {
    let tmp = Temp::new(); let (root, work) = git_fixture(&tmp);
    fs::write(work.join("uncommitted.txt"), "keep").unwrap();
    assert!(discard(&root, ID, &mut System).is_err()); assert!(work.join("uncommitted.txt").exists());
    fs::remove_file(work.join("uncommitted.txt")).unwrap();
    git(&root, &["worktree", "lock", work.to_str().unwrap()]); assert!(discard(&root, ID, &mut System).is_err());
    git(&root, &["worktree", "unlock", work.to_str().unwrap()]);
    assert!(discard(&work, ID, &mut System).is_err());
    git(&work, &["checkout", "-b", "another-branch"]); assert!(discard(&root, ID, &mut System).is_err()); assert!(work.exists());
    git(&work, &["checkout", &format!("mt-bug/{ID}")]); discard(&root, ID, &mut System).unwrap();
}
#[test]
fn discard_resolves_main_repo_when_sender_is_a_linked_lane() {
    let tmp = Temp::new(); let (root, work) = git_fixture(&tmp); let lane = tmp.0.join("_lanes/sender");
    git(&root, &["worktree", "add", "-b", "sender", lane.to_str().unwrap()]);
    discard(&lane, ID, &mut System).unwrap(); assert!(!work.exists()); assert!(lane.exists());
}
#[test]
fn dev_ui_targets_note_mode_and_fixed_actions() {
    let tmp = Temp::new(); let report = tmp.report(); let mut s = State::default(); s.saved(&report);
    assert_eq!(s.click((10, 99)), Some(Action::Send)); assert_eq!(s.click((10, 111)), Some(Action::Note)); assert_eq!(s.click((10, 123)), None);
    s.action(tmp.0.clone(), &report, Action::Accept); assert!(!s.busy());
    s.action(tmp.0.clone(), &report, Action::Note); assert!(s.editing_note);
    for c in "more detail".chars() { assert!(s.edit(mt1::ui::scene::PlusAction::Text(c))); }
    s.action(tmp.0.clone(), &report, Action::Note); settle(&mut s, &tmp.0, &report, Instant::now());
    assert!(!s.editing_note); assert!(s.note.is_empty()); assert_eq!(s.message, "Note added");
    s.remote = Some(fixed()); assert_eq!(s.click((10, 123)), Some(Action::Accept)); assert_eq!(s.click((180, 123)), Some(Action::Discard));
    let assets = crate::panels::assets::Assets::default(); let mut out = crate::panels::Surface::new(304, 180); s.draw(&assets, &mut out);
    let displayed: String = out.text.visible.iter().flat_map(|r| r.bytes.iter().map(|b| char::from(*b))).collect();
    for label in ["Send to the fixer", "Add note", "Accept", "Discard", "fixed: ready to review"] { assert!(displayed.contains(label), "{label}"); }
    assert!(out.text.visible.iter().all(|r| r.x >= 0 && r.x + i64::from(r.width) <= 304 && r.y >= 0 && r.y + 8 <= 159));
    s.resolved = true; assert_eq!(s.click((10, 123)), None);
    s.saved(&report); assert!(s.remote.is_none()); assert!(s.message.is_empty());
}

#[test]
fn review_rechecks_saved_report_root_and_status_after_cached_fixed() {
    let tmp = Temp::new(); let report = tmp.report();
    assert!(ready(&tmp.0, &report).is_err());
    atomic_json(&report.directory.join("status.json"), &json!({"id":ID,"state":"fixed","commit":"abc1234"})).unwrap();
    ready(&tmp.0, &report).unwrap();
    let other = Temp::new(); other.report(); assert!(ready(&other.0, &report).is_err());
    for action in [Action::Accept, Action::Discard] {
        let mut state = State::default(); state.saved(&report); state.remote = Some(fixed());
        atomic_json(&report.directory.join("status.json"), &json!({"id":ID,"state":"fixing"})).unwrap();
        state.action(tmp.0.clone(), &report, action); settle(&mut state, &tmp.0, &report, Instant::now());
        assert!(state.message.contains("not ready for review")); assert!(!state.resolved);
    }
}

#[test]
fn status_poll_does_not_swallow_note_typing_and_note_save_freezes_its_text() {
    use mt1::ui::scene::PlusAction as A;
    let mut state = State { editing_note: true, pending: Some(Arc::new(Mutex::new(None))), ..State::default() };
    assert!(state.busy()); state.edit(A::Text('x')); assert_eq!(state.note, "x");
    state.note = "a".repeat(1023); state.edit(A::Text('é')); assert_eq!(state.note.len(), 1023);
    state.edit(A::Backspace); state.edit(A::Text('é')); assert_eq!(state.note.len(), 1024);
    state.saving_note = true; state.edit(A::Backspace); assert_eq!(state.note.len(), 1024);
    state.edit(A::Text('\n')); assert_eq!(state.note.len(), 1024);
}

#[cfg(windows)]
#[test]
fn canonical_windows_git_arguments_preserve_disk_unc_and_unicode() {
    for (path, expected) in [(r"\\?\C:\test é\work", r"C:\test é\work"), (r"\\?\UNC\server\share\work", r"\\server\share\work"), (r"C:\work", r"C:\work")] {
        assert_eq!(git_path(Path::new(path)), OsString::from(expected));
    }
}
