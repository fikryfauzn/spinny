//! Linux publication boundary. All mutations are relative to a pinned directory.
use std::{
    ffi::{CString, OsStr},
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(super) struct Publication {
    directory: File,
    destination: CString,
    pub(super) output_path: PathBuf,
    temporary: Option<CString>,
    file: Option<File>,
    published: bool,
    leave_temporary: bool,
}
fn cstring(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}
impl Publication {
    pub(super) fn prepare(output: &Path) -> io::Result<Self> {
        let name = output.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "output filename missing")
        })?;
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = std::fs::canonicalize(parent)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(&parent)?;
        Ok(Self {
            directory,
            destination: cstring(name)?,
            output_path: parent.join(name),
            temporary: None,
            file: None,
            published: false,
            leave_temporary: false,
        })
    }
    fn anchored(&self, name: &CString) -> PathBuf {
        PathBuf::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.directory.as_raw_fd()
        ))
        .join(OsStr::from_bytes(name.as_bytes()))
    }
    pub(super) fn destination_path(&self) -> PathBuf {
        self.anchored(&self.destination)
    }
    pub(super) fn create_temp(&mut self) -> io::Result<()> {
        if self.temporary.is_some() || self.published {
            return Err(io::Error::other("temporary already created"));
        }
        for _ in 0..32 {
            let name = CString::new(format!(".vdisc-burn-{}.tmp", Uuid::new_v4()))
                .expect("UUID contains no NUL");
            // SAFETY: the directory fd and NUL-terminated name remain valid through openat.
            let fd = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDWR
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_CLOEXEC
                        | libc::O_NOFOLLOW,
                    0o600 as libc::mode_t,
                )
            };
            if fd >= 0 {
                // SAFETY: openat returned a new, uniquely owned file descriptor.
                self.file = Some(unsafe { File::from_raw_fd(fd) });
                self.temporary = Some(name);
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary filename collisions",
        ))
    }
    pub(super) fn temp_file_mut(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("no temporary file"))
    }
    pub(super) fn validation_path(&self) -> io::Result<PathBuf> {
        self.temporary
            .as_ref()
            .map(|name| self.anchored(name))
            .ok_or_else(|| io::Error::other("no temporary name"))
    }
    pub(super) fn temporary_path(&self) -> PathBuf {
        self.temporary
            .as_ref()
            .map(|name| {
                self.output_path
                    .with_file_name(OsStr::from_bytes(name.as_bytes()))
            })
            .unwrap_or_else(|| self.output_path.clone())
    }
    pub(super) fn publish(&mut self) -> io::Result<()> {
        let name = self
            .temporary
            .as_ref()
            .ok_or_else(|| io::Error::other("no temporary name"))?;
        // SAFETY: both names and the shared directory fd are live; no-replace is atomic.
        let result = unsafe {
            libc::renameat2(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                self.directory.as_raw_fd(),
                self.destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        self.published = true;
        self.temporary = None;
        Ok(())
    }
    pub(super) fn sync_directory(&self) -> io::Result<()> {
        self.directory.sync_all()
    }
    pub(super) fn leave_temporary(&mut self) {
        self.leave_temporary = true;
    }
    pub(super) fn cleanup(&mut self) -> io::Result<()> {
        if self.published {
            return Ok(());
        }
        if let Some(name) = &self.temporary {
            // SAFETY: unlink only the owned temporary basename under the pinned directory.
            let result = unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::NotFound {
                    return Err(error);
                }
            }
        }
        self.temporary = None;
        self.file = None;
        Ok(())
    }
}
impl Drop for Publication {
    fn drop(&mut self) {
        if !self.leave_temporary {
            let _ = self.cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn renamed_parent_stays_pinned_and_does_not_redirect_to_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("parent");
        let moved = dir.path().join("moved");
        std::fs::create_dir(&original).unwrap();
        let mut p = Publication::prepare(&original.join("out.vdisc")).unwrap();
        p.create_temp().unwrap();
        p.temp_file_mut().unwrap().write_all(b"complete").unwrap();
        std::fs::rename(&original, &moved).unwrap();
        std::fs::create_dir(&original).unwrap();
        assert_eq!(
            std::fs::read(p.validation_path().unwrap()).unwrap(),
            b"complete"
        );
        p.publish().unwrap();
        p.sync_directory().unwrap();
        assert_eq!(std::fs::read(moved.join("out.vdisc")).unwrap(), b"complete");
        assert!(!original.join("out.vdisc").exists());
    }
    #[test]
    fn late_existing_destination_cannot_be_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.vdisc");
        let mut p = Publication::prepare(&out).unwrap();
        p.create_temp().unwrap();
        p.temp_file_mut().unwrap().write_all(b"ours").unwrap();
        std::fs::write(&out, b"theirs").unwrap();
        assert_eq!(
            p.publish().unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        p.cleanup().unwrap();
        assert_eq!(std::fs::read(out).unwrap(), b"theirs");
    }
    #[test]
    fn drop_cleans_only_owned_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join(".vdisc-burn-other.tmp");
        std::fs::write(&other, b"other").unwrap();
        let temp;
        {
            let mut p = Publication::prepare(&dir.path().join("out.vdisc")).unwrap();
            p.create_temp().unwrap();
            temp = p.temporary_path();
            assert!(temp.exists());
        }
        assert!(!temp.exists());
        assert_eq!(std::fs::read(other).unwrap(), b"other");
    }
}
