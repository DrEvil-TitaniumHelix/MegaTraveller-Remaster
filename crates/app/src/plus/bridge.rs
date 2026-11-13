#[cfg(not(feature = "dev"))]
compile_error!("The bridge implementation requires the dev feature");
mod boundary;
mod files;
mod process;
use boundary::dev_only;
use super::bug::Report;
use serde_json::{json, Value};
use std::{
    ffi::{OsStr, OsString}, fs::{self, OpenOptions}, io::{self, Read, Write},
    path::{Path, PathBuf}, process::{Command, Stdio},
    sync::{Arc, Mutex}, thread, time::{Duration, Instant},
};

pub const AVAILABLE: bool = true;
const POLL: Duration = Duration::from_secs(5);
const LIMIT: u64 = 1024 * 1024;
const STATES: &[&str] = &["received", "reproducing", "reproduced", "fixing", "fixed", "not reproduced", "needs info", "queued"];
fn invalid(message: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message) }

pub fn repo() -> PathBuf { dev_only();
    std::env::var_os("MT_REPO").filter(|s| !s.is_empty()).map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_owned())
}

pub fn validate_id(id: &str) -> io::Result<()> { dev_only();
    if !id.is_ascii() || !(19..=25).contains(&id.len()) { return Err(invalid("Invalid report id")); }
    let b = id.as_bytes();
    if !b[..8].iter().all(u8::is_ascii_digit) || b[8] != b'-'
        || !b[9..15].iter().all(u8::is_ascii_digit) || &b[15..19] != b"-mt1" {
        return Err(invalid("Invalid report id"));
    }
    if id.len() > 19 {
        let suffix = &id[20..];
        if b[19] != b'-' || suffix.starts_with('0') || !suffix.bytes().all(|c| c.is_ascii_digit())
            || !suffix.parse::<u32>().is_ok_and(|n| (2..=10000).contains(&n)) {
            return Err(invalid("Invalid report id"));
        }
    }
    Ok(())
}

fn folder(root: &Path, report: &Report) -> io::Result<PathBuf> {
    validate_id(&report.id)?;
    let expected = root.canonicalize()?.join("acceptance/local/bugs").join(&report.id);
    if expected.canonicalize()? != expected || report.directory.canonicalize()? != expected {
        return Err(invalid("Report is outside this checkout's bug folder"));
    }
    regular(&expected.join("bundle.json"))?;
    Ok(expected)
}
fn regular(path: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(path)?.file_type().is_file() { return Err(invalid("Expected a regular report file")); }
    Ok(())
}
fn read_json(path: &Path) -> io::Result<Value> { dev_only();
    regular(path)?;
    let mut bytes = Vec::new(); fs::File::open(path)?.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT { return Err(invalid("Report file is too large")); }
    Ok(serde_json::from_slice(&bytes)?)
}
fn write_json(path: &Path, value: &Value, replace: bool) -> io::Result<()> { dev_only();
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let result = (|| {
        f.write_all(&serde_json::to_vec(value)?)?; f.sync_all()?; drop(f);
        if replace { fs::rename(&tmp, path) }
        else {
            match fs::hard_link(&tmp, path) { Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()), result => result }
        }
    })();
    let _ = fs::remove_file(&tmp);
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool { Herdr, Git }
trait Runner { fn run(&mut self, tool: Tool, root: &Path, args: &[OsString]) -> io::Result<String>; }
struct System;
impl Runner for System {
    fn run(&mut self, tool: Tool, root: &Path, args: &[OsString]) -> io::Result<String> {
        let program = match (tool, cfg!(windows)) { (Tool::Herdr, true) => "herdr.exe", (Tool::Herdr, false) => "herdr", (Tool::Git, true) => "git.exe", (Tool::Git, false) => "git" };
        let mut command = Command::new(program);
        command.current_dir(root).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        dev_only();
        if tool == Tool::Git { process::sanitize_git(&mut command); }
        let output = process::output(&mut command, Duration::from_secs(15))?;
        if !output.status.success() { return Err(io::Error::other(format!("{program}: {}", super::one_line(&String::from_utf8_lossy(&output.stderr))))); }
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        if tool == Tool::Herdr {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if value.get("error").is_some_and(|v| !v.is_null()) || value.get("ok") == Some(&Value::Bool(false)) {
                    return Err(io::Error::other("Herdr rejected the request"));
                }
            }
        }
        Ok(text)
    }
}
fn run(runner: &mut impl Runner, tool: Tool, root: &Path, args: &[&str]) -> io::Result<String> {
    runner.run(tool, root, &args.iter().map(OsString::from).collect::<Vec<_>>())
}
fn pane_ok(pane: &str) -> bool {
    !pane.is_empty() && pane.len() <= 80 && !pane.starts_with('-')
        && pane.bytes().all(|c| c.is_ascii_alphanumeric() || b":_-".contains(&c))
}
fn pane_from_list(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let agents = v.pointer("/result/agents").or_else(|| v.get("agents"))?.as_array()?;
    agents.iter().find_map(|a| {
        let pane = a.get("pane_id")?.as_str()?;
        (a.get("name")?.as_str()? == "mtforeman" && pane_ok(pane)).then(|| pane.to_owned())
    })
}
fn ping(id: &str, bundle: &Value) -> io::Result<String> {
    validate_id(id)?;
    if bundle.get("id").and_then(Value::as_str) != Some(id) { return Err(invalid("Bundle id does not match saved report")); }
    let kind = bundle.get("kind").and_then(Value::as_str).filter(|k| ["bug", "qol", "question"].contains(k)).ok_or_else(|| invalid("Invalid report kind"))?;
    let text = bundle.get("text").and_then(Value::as_str).ok_or_else(|| invalid("Missing report text"))?;
    let summary: String = text.chars().map(|c| if c.is_control() || c == '\u{2028}' || c == '\u{2029}' { ' ' } else { c }).take(80).collect();
    Ok(format!("BUGBRIDGE v1 {id} {kind} acceptance/local/bugs/{id}/bundle.json :: {summary}"))
}
fn send(root: &Path, report: &Report, fallback: Option<&str>, runner: &mut impl Runner) -> io::Result<String> { dev_only();
    let dir = folder(root, report)?;
    let line = ping(&report.id, &read_json(&dir.join("bundle.json"))?)?;
    let pane = run(runner, Tool::Herdr, root, &["agent", "list"]).ok().and_then(|s| pane_from_list(&s))
        .or_else(|| fallback.filter(|s| pane_ok(s)).map(str::to_owned));
    if let Some(pane) = pane {
        run(runner, Tool::Herdr, root, &["pane", "send-text", &pane, &line])?;
        run(runner, Tool::Herdr, root, &["pane", "send-keys", &pane, "enter"])?;
        return Ok("Sent; waiting for the fixer".into());
    }
    let path = dir.join("status.json");
    if !path.try_exists()? {
        write_json(&path, &json!({"id":report.id,"state":"queued","reason":"no foreman pane", "at":super::now(),"lane":null,"message":"no foreman pane"}), false)?;
    }
    Ok("queued: no foreman pane".into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status { pub state: String, pub message: String, pub commit: Option<String> }
fn status(root: &Path, report: &Report) -> io::Result<Option<Status>> { dev_only();
    let path = folder(root, report)?.join("status.json");
    if !path.try_exists()? { return Ok(None); }
    let v = read_json(&path)?;
    if v.get("id").is_some_and(|id| id.as_str() != Some(&report.id)) { return Err(invalid("Status belongs to a different report")); }
    let state = v.get("state").and_then(Value::as_str).filter(|s| STATES.contains(s)).ok_or_else(|| invalid("Invalid fixer status"))?;
    let message = v.get("message").or_else(|| v.get("reason")).and_then(Value::as_str).unwrap_or("");
    let commit = v.get("commit").and_then(Value::as_str).map(str::to_owned);
    if state == "fixed" && !commit.as_ref().is_some_and(|c| (7..=64).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err(invalid("Fixed status needs a commit"));
    }
    Ok(Some(Status { state: state.into(), message: super::one_line(message), commit }))
}
fn note(root: &Path, report: &Report, text: &str) -> io::Result<()> { dev_only();
    let text = super::one_line(text);
    if text.is_empty() { return Err(invalid("Type a note first")); }
    let path = folder(root, report)?.join("notes.jsonl");
    let mut bytes = serde_json::to_vec(&json!({"at":super::now(),"text":text}))?; bytes.push(b'\n');
    files::append(&path, &bytes, || {}, || {})
}
fn ready(root: &Path, report: &Report) -> io::Result<()> {
    if !status(root, report)?.is_some_and(|s| s.state == "fixed") {
        return Err(invalid("Fix is not ready for review"));
    }
    Ok(())
}

fn branch(id: &str) -> io::Result<String> { validate_id(id)?; Ok(format!("mt-bug/{id}")) }
fn repository(root: &Path, runner: &mut impl Runner) -> io::Result<PathBuf> {
    let root = root.canonicalize()?;
    let dot = root.join(".git");
    let metadata = fs::symlink_metadata(&dot)?;
    let (gitdir, common) = if metadata.is_dir() {
        let common = dot.canonicalize()?;
        if common != dot { return Err(invalid("Redirected Git directory")); }
        (common.clone(), common)
    } else {
        regular(&dot)?;
        let pointer = fs::read_to_string(&dot)?;
        let target = pointer.trim().strip_prefix("gitdir: ").ok_or_else(|| invalid("Invalid checkout gitfile"))?;
        let gitdir = root.join(target).canonicalize()?;
        let common = gitdir.join(fs::read_to_string(gitdir.join("commondir"))?.trim()).canonicalize()?;
        if Path::new(fs::read_to_string(gitdir.join("gitdir"))?.trim()).canonicalize()? != dot {
            return Err(invalid("Git worktree backlink does not match checkout"));
        }
        (gitdir, common)
    };
    let answer = run(runner, Tool::Git, &root, &["rev-parse", "--path-format=absolute", "--show-toplevel", "--absolute-git-dir", "--git-common-dir"])?;
    let paths: Vec<_> = answer.lines().map(|line| Path::new(line).canonicalize()).collect::<io::Result<_>>()?;
    if paths != [root, gitdir, common.clone()] { return Err(invalid("Git repository identity does not match bundle checkout")); }
    Ok(common)
}
fn git_path(path: &Path) -> OsString {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let chars: Vec<u16> = path.as_os_str().encode_wide().collect();
        if chars.starts_with(&[92, 92, 63, 92, 85, 78, 67, 92]) { return OsString::from_wide(&[&[92, 92][..], &chars[8..]].concat()); }
        if chars.starts_with(&[92, 92, 63, 92]) { return OsString::from_wide(&chars[4..]); }
    }
    path.as_os_str().to_owned()
}
fn accept(root: &Path, id: &str, runner: &mut impl Runner) -> io::Result<String> { dev_only();
    let name = branch(id)?;
    repository(root, runner)?;
    run(runner, Tool::Git, root, &["show-ref", "--verify", &format!("refs/heads/{name}")])?;
    Ok(format!("git merge {name}"))
}
fn discard(root: &Path, id: &str, runner: &mut impl Runner) -> io::Result<()> { dev_only();
    let name = branch(id)?;
    let common = repository(root, runner)?;
    if common.file_name() != Some(OsStr::new(".git")) { return Err(invalid("Expected a non-bare checkout")); }
    let main = common.parent().ok_or_else(|| invalid("Missing main checkout"))?;
    let lanes = main.parent().ok_or_else(|| invalid("Missing checkout parent"))?.join("_lanes");
    let expected = lanes.join(format!("MegaTraveller-Remaster-bug-{id}"));
    if lanes.canonicalize()? != lanes || expected.canonicalize()? != expected || root.canonicalize()? == expected {
        return Err(invalid("Fix worktree path is redirected or is the current checkout"));
    }
    let listing = run(runner, Tool::Git, root, &["worktree", "list", "--porcelain", "-z"])?;
    let mut matched = false;
    for record in listing.split("\0\0") {
        let fields: Vec<_> = record.split('\0').collect();
        let path = fields.iter().find_map(|s| s.strip_prefix("worktree "));
        if path.is_some_and(|p| Path::new(p).canonicalize().ok().as_ref() == Some(&expected)) {
            if !fields.contains(&format!("branch refs/heads/{name}").as_str()) || fields.iter().any(|s| s.starts_with("locked")) {
                return Err(invalid("Fix worktree is locked or has a different branch"));
            }
            matched = true;
        }
    }
    if !matched { return Err(invalid("Expected fix worktree is not registered")); }
    if repository(&expected, runner)? != common { return Err(invalid("Fix worktree belongs to another repository")); }
    let dirty = run(runner, Tool::Git, &expected, &["-c", "status.showUntrackedFiles=all", "status", "--porcelain=v1", "-z", "-uall", "--ignored=no", "--ignore-submodules=none"])?;
    if !dirty.is_empty() { return Err(invalid("Fix worktree has uncommitted or untracked files")); }
    runner.run(Tool::Git, root, &["-c".into(), "status.showUntrackedFiles=all".into(), "worktree".into(), "remove".into(), "--".into(), git_path(&expected)])?;
    run(runner, Tool::Git, root, &["branch", "-D", "--", &name])?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Send, Note, Accept, Discard }
#[derive(Debug)]
enum Reply { Message(String), Note, Accepted(String), Discarded, Status(Option<Status>) }
type Pending = Arc<Mutex<Option<io::Result<Reply>>>>;
#[derive(Clone, Debug, Default)]
pub struct State {
    pub editing_note: bool, pub note: String, pub remote: Option<Status>, pub message: String,
    report_id: String, last_poll: Option<Instant>, pending: Option<Pending>, resolved: bool, saving_note: bool,
}
impl State {
    pub fn saved(&mut self, report: &Report) { dev_only(); *self = Self { report_id: report.id.clone(), ..Self::default() }; }
    pub fn fixed(&self) -> bool { dev_only(); !self.resolved && self.remote.as_ref().is_some_and(|s| s.state == "fixed") }
    pub fn busy(&self) -> bool { dev_only(); self.pending.is_some() }
    pub fn edit(&mut self, action: mt1::ui::scene::PlusAction) -> bool { dev_only();
        use mt1::ui::scene::PlusAction as A;
        if !self.editing_note { return false; }
        match action {
            A::Text(c) => { if !self.saving_note && !c.is_control() && self.note.len() + c.len_utf8() <= 1024 { self.note.push(c); } }
            A::Backspace => { if !self.saving_note { self.note.pop(); } }
            _ => return false,
        }
        true
    }
    fn spawn(&mut self, work: impl FnOnce() -> io::Result<Reply> + Send + 'static) {
        let result = Arc::new(Mutex::new(None)); self.pending = Some(result.clone());
        thread::spawn(move || { let reply = work(); *result.lock().unwrap() = Some(reply); });
    }
    pub fn action(&mut self, root: PathBuf, report: &Report, action: Action) { dev_only();
        if self.report_id != report.id { self.saved(report); }
        if self.busy() { return; }
        if action == Action::Note && !self.editing_note { self.editing_note = true; return; }
        if matches!(action, Action::Accept | Action::Discard) && !self.fixed() { return; }
        let report = report.clone(); let text = self.note.clone();
        self.saving_note = action == Action::Note;
        self.message = "Working...".into();
        self.spawn(move || match action {
            Action::Send => send(&root, &report, std::env::var("MT_FOREMAN_PANE").ok().as_deref(), &mut System).map(Reply::Message),
            Action::Note => { note(&root, &report, &text)?; Ok(Reply::Note) }
            Action::Accept => { ready(&root, &report)?; accept(&root, &report.id, &mut System).map(Reply::Accepted) }
            Action::Discard => { ready(&root, &report)?; discard(&root, &report.id, &mut System)?; Ok(Reply::Discarded) }
        });
    }
    pub fn poll(&mut self, root: PathBuf, report: &Report, now: Instant) -> bool { dev_only();
        if self.report_id != report.id { self.saved(report); }
        let reply = self.pending.as_ref().and_then(|p| p.lock().unwrap().take());
        let changed = reply.is_some();
        if let Some(reply) = reply {
            self.pending = None; self.saving_note = false;
            match reply {
                Ok(Reply::Status(s)) => self.remote = s,
                Ok(Reply::Message(s)) => { self.message = s; self.last_poll = None; }
                Ok(Reply::Note) => { self.note.clear(); self.editing_note = false; self.message = "Note added".into(); }
                Ok(Reply::Accepted(line)) => { self.resolved = true; self.message = line; }
                Ok(Reply::Discarded) => { self.resolved = true; self.message = "Fix branch discarded".into(); }
                Err(e) => { self.message = format!("Fixer: {e}"); self.remote = None; }
            }
        }
        if !self.busy() && self.last_poll.is_none_or(|t| now.saturating_duration_since(t) >= POLL) {
            self.last_poll = Some(now); let report = report.clone();
            self.spawn(move || status(&root, &report).map(Reply::Status));
        }
        changed
    }
    pub fn click(&self, at: (i32, i32)) -> Option<Action> { dev_only();
        if !(4..300).contains(&at.0) { return None; }
        match at.1 {
            218..=227 => Some(Action::Send), 228..=237 => Some(Action::Note),
            238..=247 if self.fixed() => Some(if at.0 < 145 { Action::Accept } else { Action::Discard }), _ => None,
        }
    }
    pub fn draw(&self, assets: &crate::panels::assets::Assets, out: &mut crate::panels::Surface) { dev_only();
        let color = if self.busy() { 7 } else { 14 };
        assets.text(out, "Send to the fixer  F6", (4, 219), color);
        assets.text(out, "Add note  F7", (4, 229), color);
        if self.fixed() {
            assets.text(out, "Accept  F8", (4, 239), color);
            assets.text(out, "Discard  F9", (145, 239), color);
        } else {
            let remote = self.remote.as_ref().map(|s| format!("{}: {}", s.state, s.message)).unwrap_or_else(|| "Awaiting fixer status".into());
            if let Some(line) = super::help::lines(&remote, 52).first() { assets.text(out, line, (4, 239), 8); }
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(all(test, windows))]
mod security_tests;
