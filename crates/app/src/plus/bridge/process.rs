use super::*;

pub(super) fn sanitize_git(command: &mut Command) {
    dev_only();
    let explicit: Vec<_> = command.get_envs().map(|(k, _)| k.to_owned()).collect();
    for key in std::env::vars_os().map(|(k, _)| k).chain(explicit) {
        if key.to_string_lossy().to_ascii_uppercase().starts_with("GIT_") { command.env_remove(key); }
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::{io::AsRawHandle, process::CommandExt};
    use windows_sys::Win32::{Foundation::*, System::{Diagnostics::ToolHelp::*, JobObjects::*, Pipes::PeekNamedPipe, Threading::*}};

    struct Handle(HANDLE);
    impl Drop for Handle { fn drop(&mut self) { unsafe { CloseHandle(self.0); } } }
    impl Handle {
        fn checked(raw: HANDLE) -> io::Result<Self> {
            if raw.is_null() || raw == INVALID_HANDLE_VALUE { Err(io::Error::last_os_error()) } else { Ok(Self(raw)) }
        }
    }
    struct Job(Handle);
    impl Job {
        fn new() -> io::Result<Self> {
            let job = Self(Handle::checked(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })?);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if unsafe { SetInformationJobObject(job.0.0, JobObjectExtendedLimitInformation,
                &info as *const _ as _, std::mem::size_of_val(&info) as u32) } == 0 { return Err(io::Error::last_os_error()); }
            Ok(job)
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe { TerminateJobObject(self.0.0, 1); }
        }
    }
    fn resume(pid: u32) -> io::Result<()> {
        let snapshot = Handle::checked(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() }; entry.dwSize = std::mem::size_of_val(&entry) as u32;
        let mut found = unsafe { Thread32First(snapshot.0, &mut entry) };
        while found != 0 {
            if entry.th32OwnerProcessID == pid {
                let thread = Handle::checked(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })?;
                if unsafe { ResumeThread(thread.0) } == u32::MAX { return Err(io::Error::last_os_error()); }
                return Ok(());
            }
            found = unsafe { Thread32Next(snapshot.0, &mut entry) };
        }
        Err(io::Error::other("Suspended command thread not found"))
    }
    fn drain(pipe: &mut (impl Read + AsRawHandle), bytes: &mut Vec<u8>) -> io::Result<bool> {
        let mut available = 0;
        if unsafe { PeekNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut available, std::ptr::null_mut()) } == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) { return Ok(true); }
            return Err(error);
        }
        if available > 0 {
            let mut buffer = [0; 8192];
            let count = pipe.read(&mut buffer[..(available as usize).min(8192)])?;
            bytes.extend_from_slice(&buffer[..count.min((LIMIT as usize).saturating_sub(bytes.len()))]);
        }
        Ok(false)
    }
    pub(super) fn output(command: &mut Command, timeout: Duration) -> io::Result<std::process::Output> {
        dev_only();
        let deadline = Instant::now() + timeout;
        let job = Job::new()?;
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        if unsafe { AssignProcessToJobObject(job.0.0, child.as_raw_handle()) } == 0 {
            let error = io::Error::last_os_error(); let _ = child.kill(); let _ = child.wait(); return Err(error);
        }
        resume(child.id())?;
        let mut stdout = child.stdout.take().unwrap(); let mut stderr = child.stderr.take().unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let (mut out_done, mut err_done) = (false, false);
        loop {
            if Instant::now() >= deadline { return Err(io::Error::new(io::ErrorKind::TimedOut, "Bridge command timed out")); }
            if !out_done { out_done = drain(&mut stdout, &mut out)?; }
            if !err_done { err_done = drain(&mut stderr, &mut err)?; }
            if let Some(status) = child.try_wait()? {
                if !status.success() || (out_done && err_done) { return Ok(std::process::Output { status, stdout: out, stderr: err }); }
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
pub(super) fn output(command: &mut Command, timeout: Duration) -> io::Result<std::process::Output> {
    dev_only();
    #[cfg(windows)]
    { windows::output(command, timeout) }
    #[cfg(not(windows))]
    { let _ = (command, timeout); Err(io::Error::new(io::ErrorKind::Unsupported, "Supervised bridge commands require Windows")) }
}
