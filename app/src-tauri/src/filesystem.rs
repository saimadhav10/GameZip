use std::{
    fs::{File, Metadata},
    io,
    path::Path,
};

pub fn is_link(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// A stable identity avoids charging hard-linked files twice within a scan.
pub fn identity(file: &File) -> io::Result<String> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // The handle is owned by `file` and the output points to valid initialized storage.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(format!(
            "{}:{}:{}",
            info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
        ))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata()?;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
}

pub fn allocated(path: &Path, _metadata: &Metadata, _file: &File) -> io::Result<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::{ffi::OsStrExt, fs::MetadataExt, io::AsRawHandle};
        use windows_sys::Win32::{
            Foundation::{GetLastError, SetLastError},
            Storage::FileSystem::{
                FILE_STANDARD_INFO, FileStandardInfo, GetCompressedFileSizeW,
                GetFileInformationByHandleEx,
            },
        };
        // Ordinary files need allocation size: GetCompressedFileSizeW returns
        // logical size for them, omitting cluster slack. Sparse/compressed files
        // instead need their actual occupied storage from GetCompressedFileSizeW.
        if _metadata.file_attributes() & (0x200 | 0x800) == 0 {
            let mut info: FILE_STANDARD_INFO = unsafe { std::mem::zeroed() };
            // The live file handle and correctly sized output buffer remain valid.
            if unsafe {
                GetFileInformationByHandleEx(
                    _file.as_raw_handle(),
                    FileStandardInfo,
                    (&mut info as *mut FILE_STANDARD_INFO).cast(),
                    std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
            return u64::try_from(info.AllocationSize)
                .map_err(|_| io::Error::other("Invalid allocation size"));
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut high = 0u32;
        // NUL-terminated path and a valid output pointer; disambiguate a valid all-ones result.
        unsafe {
            SetLastError(0);
            let low = GetCompressedFileSizeW(wide.as_ptr(), &mut high);
            let error = GetLastError();
            if low == u32::MAX && error != 0 {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            Ok((u64::from(high) << 32) | u64::from(low))
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = path;
        Ok(_metadata.blocks() * 512)
    }
}

pub fn root(path: &str) -> Result<std::path::PathBuf, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| format!("Cannot open folder: {e}"))?;
    if !canonical.is_dir() {
        return Err("Choose a folder, not a file.".into());
    }
    Ok(canonical)
}

pub fn display(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}
