use super::*;

#[cfg(windows)]
pub(super) struct Pinned(Vec<fs::File>);
#[cfg(windows)]
impl Pinned {
    pub(super) fn directory(path: &Path) -> io::Result<Self> {
        dev_only();
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::*;
        let mut pinned = Self(Vec::new());
        let mut ancestors: Vec<_> = path.ancestors().collect(); ancestors.reverse();
        for ancestor in ancestors {
            let file = OpenOptions::new().read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).open(ancestor)?;
            let meta = file.metadata()?;
            if !meta.is_dir() || meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(invalid("Redirected note directory"));
            }
            pinned.0.push(file);
        }
        Ok(pinned)
    }
}

#[cfg(windows)]
pub(super) fn append(path: &Path, bytes: &[u8], before_open: impl FnOnce(), after_open: impl FnOnce()) -> io::Result<()> {
    dev_only();
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::*;
    let _parents = Pinned::directory(path.parent().ok_or_else(|| invalid("Missing note directory"))?)?;
    before_open();
    let mut file = OpenOptions::new().append(true).create(true).share_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
    after_open();
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 { return Err(io::Error::last_os_error()); }
    if info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
        || info.nNumberOfLinks != 1 {
        return Err(invalid("Notes must be a regular file with exactly one link"));
    }
    file.write_all(bytes)
}

#[cfg(not(windows))]
pub(super) fn append(_path: &Path, _bytes: &[u8], _before_open: impl FnOnce(), _after_open: impl FnOnce()) -> io::Result<()> {
    dev_only();
    Err(io::Error::new(io::ErrorKind::Unsupported, "Protected bridge notes require Windows"))
}
