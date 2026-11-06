//! Windows LZX operations. The caller owns the durable job journal and schedules
//! one file at a time. No recursive compact command or CompactOS switch is used.
use crate::filesystem;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Compress,
    Decompress,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Strength {
    Fast,
    Balanced,
    #[default]
    Maximum,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    pub relative: PathBuf,
    pub identity: String,
    pub logical: u64,
    pub allocated: u64,
    pub modified: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default)]
    pub strength: Strength,
    pub root: PathBuf,
    pub operation: Operation,
    pub files: Vec<PlannedFile>,
    pub skipped: u64,
    pub logical: u64,
    pub allocated: u64,
    pub free: u64,
    pub required_free: u64,
}

#[derive(Debug)]
pub struct Outcome {
    pub before: u64,
    pub after: u64,
    pub changed: bool,
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        io,
        os::windows::{
            ffi::{OsStrExt, OsStringExt},
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
            process::CommandExt,
        },
        process::Command,
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError},
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_PROVIDER_COMPRESSION_LZX, FILE_PROVIDER_COMPRESSION_XPRESS4K,
            FILE_PROVIDER_COMPRESSION_XPRESS8K, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
            FILE_SHARE_WRITE, GetDiskFreeSpaceExW, GetFileInformationByHandle,
            GetVolumeInformationW, GetVolumePathNameW, WOF_FILE_COMPRESSION_INFO_V1,
            WOF_PROVIDER_FILE, WofIsExternalFile,
        },
        System::{
            ProcessStatus::EnumProcesses,
            SystemInformation::GetSystemDirectoryW,
            Threading::{
                BELOW_NORMAL_PRIORITY_CLASS, CREATE_NO_WINDOW, OpenProcess,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
    };

    const RESERVE: u64 = 64 * 1024 * 1024;
    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }
    fn os_error() -> String {
        io::Error::last_os_error().to_string()
    }

    pub fn free_space(root: &Path) -> Result<u64, String> {
        let path = wide(root);
        let mut free = 0;
        // NUL-terminated path and valid output storage; optional outputs are null.
        if unsafe {
            GetDiskFreeSpaceExW(path.as_ptr(), &mut free, ptr::null_mut(), ptr::null_mut())
        } == 0
        {
            return Err(os_error());
        }
        Ok(free)
    }

    fn require_ntfs(root: &Path) -> Result<(), String> {
        let path = wide(root);
        let mut volume = vec![0u16; 32768];
        let mut name = [0u16; 64];
        // Buffers are writable and their lengths are passed in UTF-16 code units.
        unsafe {
            if GetVolumePathNameW(path.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) == 0 {
                return Err(os_error());
            }
            if GetVolumeInformationW(
                volume.as_ptr(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                name.as_mut_ptr(),
                name.len() as u32,
            ) == 0
            {
                return Err(os_error());
            }
        }
        let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
        if String::from_utf16_lossy(&name[..end]) != "NTFS" {
            return Err("LZX requires an NTFS drive.".into());
        }
        Ok(())
    }

    /// None = ordinary file; Some(algorithm) = WOF file provider. Other backing
    /// providers are rejected rather than changing externally managed storage.
    fn algorithm(path: &Path) -> Result<Option<u32>, String> {
        let path = wide(path);
        let mut external = 0;
        let mut provider = 0;
        let mut info = WOF_FILE_COMPRESSION_INFO_V1::default();
        let mut size = std::mem::size_of_val(&info) as u32;
        // Correctly sized provider information and output pointers.
        let hr = unsafe {
            WofIsExternalFile(
                path.as_ptr(),
                &mut external,
                &mut provider,
                (&mut info as *mut WOF_FILE_COMPRESSION_INFO_V1).cast(),
                &mut size,
            )
        };
        if hr < 0 {
            return Err(format!("Cannot inspect compression state ({hr:#x})."));
        }
        if external == 0 {
            return Ok(None);
        }
        if provider != WOF_PROVIDER_FILE || size as usize > std::mem::size_of_val(&info) {
            return Err("File uses unsupported external storage.".into());
        }
        Ok(Some(info.Algorithm))
    }

    fn eligible(
        path: &Path,
        file: &fs::File,
        metadata: &fs::Metadata,
        op: Operation,
    ) -> Result<bool, String> {
        // Read-only, encrypted, sparse, NTFS-compressed and reparse files are
        // excluded. WOF normally exposes ordinary attributes through its filter.
        if metadata.file_attributes() & (0x1 | 0x4000 | 0x200 | 0x800 | 0x400) != 0 {
            return Ok(false);
        }
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // Valid open handle and correctly sized output buffer.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
            return Err(os_error());
        }
        // A hard link can point outside the selected game and share its storage.
        if info.nNumberOfLinks != 1 {
            return Ok(false);
        }
        Ok(match (op, algorithm(path)?) {
            (Operation::Compress, None) => metadata.len() > 0,
            (
                Operation::Decompress,
                Some(
                    FILE_PROVIDER_COMPRESSION_LZX
                    | FILE_PROVIDER_COMPRESSION_XPRESS4K
                    | FILE_PROVIDER_COMPRESSION_XPRESS8K,
                ),
            ) => true,
            _ => false,
        })
    }

    fn modified(metadata: &fs::Metadata) -> Result<u128, String> {
        Ok(metadata
            .modified()
            .map_err(|e| e.to_string())?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos())
    }

    // compact.exe rejects verbatim paths. Reject names that Win32 would
    // normalise to a different target when the verbatim prefix is removed.
    fn compact_path(path: &Path) -> Result<String, String> {
        let text = path
            .to_str()
            .ok_or("This filename cannot be passed to compact.exe.")?;
        let plain = text.strip_prefix(r"\\?\").unwrap_or(text);
        if plain.starts_with("UNC\\")
            || plain.starts_with(r"\\")
            || plain.encode_utf16().count() >= 260
        {
            return Err(
                "Compression currently requires a local path shorter than 260 characters.".into(),
            );
        }
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                let name = name.to_str().ok_or("Unsupported filename.")?;
                let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                if name.ends_with([' ', '.'])
                    || name.contains(['*', '?', ':'])
                    || ["CON", "PRN", "AUX", "NUL", "CLOCK$"].contains(&base.as_str())
                    || (base.len() == 4
                        && (base.starts_with("COM") || base.starts_with("LPT"))
                        && base.as_bytes()[3].is_ascii_digit())
                {
                    return Err(
                        "Filename would be interpreted differently by Windows. File excluded."
                            .into(),
                    );
                }
            }
        }
        Ok(plain.to_string())
    }

    pub fn prepare(root: &Path, operation: Operation, cancel: &AtomicBool) -> Result<Plan, String> {
        let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        if root.parent().is_none() {
            return Err("Choose a game folder, not an entire drive.".into());
        }
        compact_path(&root)?;
        require_ntfs(&root)?;
        ensure_game_stopped(&root)?;
        let mut plan = Plan {
            strength: Strength::default(),
            root: root.clone(),
            operation,
            files: Vec::new(),
            skipped: 0,
            logical: 0,
            allocated: 0,
            free: free_space(&root)?,
            required_free: RESERVE,
        };
        let mut directories = vec![root.clone()];
        let mut largest = 0;
        let mut expansion = 0u64;
        while let Some(directory) = directories.pop() {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(_) => {
                    plan.skipped += 1;
                    continue;
                }
            };
            for entry in entries {
                if cancel.load(Ordering::Relaxed) {
                    return Err("cancelled".into());
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => {
                        plan.skipped += 1;
                        continue;
                    }
                };
                let path = entry.path();
                let inspect = || -> Result<Option<PlannedFile>, String> {
                    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                    if filesystem::is_link(&metadata) {
                        return Ok(None);
                    }
                    if metadata.is_dir() {
                        return Ok(None);
                    }
                    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
                    compact_path(&path)?;
                    if !metadata.is_file() || !eligible(&path, &file, &metadata, operation)? {
                        return Ok(None);
                    }
                    Ok(Some(PlannedFile {
                        relative: path
                            .strip_prefix(&root)
                            .map_err(|e| e.to_string())?
                            .to_path_buf(),
                        identity: filesystem::identity(&file).map_err(|e| e.to_string())?,
                        logical: metadata.len(),
                        allocated: filesystem::allocated(&path, &metadata, &file)
                            .map_err(|e| e.to_string())?,
                        modified: modified(&metadata)?,
                    }))
                };
                let metadata = fs::symlink_metadata(&path);
                if metadata
                    .as_ref()
                    .is_ok_and(|m| m.is_dir() && !filesystem::is_link(m))
                {
                    directories.push(path);
                    continue;
                }
                match inspect() {
                    Ok(Some(file)) => {
                        largest = largest.max(file.logical);
                        // Reserve an extra cluster per file rather than underestimating
                        // expansion due to the final partial cluster.
                        expansion = expansion.saturating_add(
                            file.logical
                                .saturating_add(65536)
                                .saturating_sub(file.allocated),
                        );
                        plan.logical = plan.logical.saturating_add(file.logical);
                        plan.allocated = plan.allocated.saturating_add(file.allocated);
                        plan.files.push(file);
                    }
                    _ => plan.skipped += 1,
                }
            }
        }
        plan.required_free =
            RESERVE
                .saturating_add(largest)
                .saturating_add(if operation == Operation::Decompress {
                    expansion
                } else {
                    0
                });
        Ok(plan)
    }

    /// Query accessible process image paths. This cannot prove that protected
    /// processes or an external launcher never hold a game file; file locking
    /// and compact's failure status remain necessary additional guards.
    pub fn running_in(root: &Path) -> Result<Vec<String>, String> {
        let mut pids = vec![0u32; 1024];
        loop {
            let mut bytes = 0;
            // Buffer length exactly matches allocated u32 storage.
            if unsafe { EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut bytes) } == 0
            {
                return Err(os_error());
            }
            if (bytes as usize) < pids.len() * 4 {
                pids.truncate(bytes as usize / 4);
                break;
            }
            if pids.len() >= 65536 {
                return Err("Could not read the complete process list.".into());
            }
            pids.resize(pids.len() * 2, 0);
        }
        let canonical_root = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let root_key = filesystem::display(&canonical_root).to_lowercase();
        let mut found = Vec::new();
        for pid in pids {
            if pid == 0 || pid == 4 {
                continue;
            }
            // Read-only process query; never stop or change a game process.
            let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
            if handle.is_null() {
                continue;
            }
            let mut name = vec![0u16; 32768];
            let mut len = name.len() as u32;
            let ok = unsafe { QueryFullProcessImageNameW(handle, 0, name.as_mut_ptr(), &mut len) };
            unsafe {
                CloseHandle(handle);
            }
            if ok == 0 {
                continue;
            }
            let path = PathBuf::from(std::ffi::OsString::from_wide(&name[..len as usize]));
            let path = fs::canonicalize(&path).unwrap_or(path);
            if Path::new(&filesystem::display(&path).to_lowercase())
                .starts_with(Path::new(&root_key))
            {
                found.push(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                );
            }
        }
        found.sort();
        found.dedup();
        Ok(found)
    }

    pub fn ensure_game_stopped(root: &Path) -> Result<(), String> {
        let processes = running_in(root)?;
        if processes.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Close the game before continuing: {}",
                processes.join(", ")
            ))
        }
    }

    /// Hold each ancestor against rename/deletion while compact uses the path.
    fn guard_path(root: &Path, relative: &Path) -> Result<Vec<fs::File>, String> {
        if relative
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            return Err("Invalid file path.".into());
        }
        let mut guards = Vec::new();
        let mut directory = root.to_path_buf();
        let mut dirs: Vec<_> = directory.ancestors().map(Path::to_path_buf).collect();
        dirs.reverse();
        if let Some(parent) = relative.parent() {
            for component in parent.components() {
                directory.push(component);
                dirs.push(directory.clone());
            }
        }
        for path in dirs {
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if !metadata.is_dir() || filesystem::is_link(&metadata) {
                return Err("Folder changed or became a link. Prepare the job again.".into());
            }
            let handle = fs::OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(&path)
                .map_err(|e| e.to_string())?;
            if filesystem::is_link(&handle.metadata().map_err(|e| e.to_string())?) {
                return Err("Folder became a link. Prepare the job again.".into());
            }
            guards.push(handle);
        }
        Ok(guards)
    }

    pub fn apply(
        root: &Path,
        operation: Operation,
        planned: &PlannedFile,
    ) -> Result<Outcome, String> {
        apply_using(root, operation, planned, Strength::Maximum)
    }

    pub fn apply_using(
        root: &Path,
        operation: Operation,
        planned: &PlannedFile,
        strength: Strength,
    ) -> Result<Outcome, String> {
        ensure_game_stopped(root)?;
        let _directories = guard_path(root, &planned.relative)?;
        let path = root.join(&planned.relative);
        let command_path = compact_path(&path)?;
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if filesystem::is_link(&metadata) || !metadata.is_file() {
            return Err("File changed or became a link. Prepare the job again.".into());
        }
        let file = fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)
            .map_err(|e| e.to_string())?;
        if filesystem::identity(&file).map_err(|e| e.to_string())? != planned.identity
            || metadata.len() != planned.logical
            || modified(&metadata)? != planned.modified
        {
            return Err("File changed since preparation. Prepare the job again.".into());
        }
        if !eligible(&path, &file, &metadata, operation)? {
            return Err("File is no longer eligible. Prepare the job again.".into());
        }
        if free_space(root)? < planned.logical.saturating_add(RESERVE) {
            return Err("Not enough free space to safely process the next file.".into());
        }
        let before = filesystem::allocated(&path, &metadata, &file).map_err(|e| e.to_string())?;
        let mut system = vec![0u16; 32768];
        // Fetch the OS system directory, without searching PATH for compact.exe.
        let len = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) };
        if len == 0 || len as usize >= system.len() {
            return Err(format!("Cannot locate compact.exe ({})", unsafe {
                GetLastError()
            }));
        }
        let compact = PathBuf::from(std::ffi::OsString::from_wide(&system[..len as usize]))
            .join("compact.exe");
        let output = Command::new(compact)
            .args(match operation {
                Operation::Compress => [
                    "/C",
                    match strength {
                        Strength::Fast => "/EXE:XPRESS4K",
                        Strength::Balanced => "/EXE:XPRESS8K",
                        Strength::Maximum => "/EXE:LZX",
                    },
                    "/A",
                ],
                Operation::Decompress => ["/U", "/EXE", "/A"],
            })
            .arg(command_path)
            .creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            #[cfg(test)]
            eprintln!(
                "compact stdout: {}; stderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(format!(
                "Windows could not process this file (exit {}). It may be in use or access may be denied.",
                output.status
            ));
        }
        let after_meta = file.metadata().map_err(|e| e.to_string())?;
        if after_meta.len() != planned.logical {
            return Err(
                "File size changed while processing. Stop and verify the installation.".into(),
            );
        }
        let state = algorithm(&path)?;
        let changed = match operation {
            Operation::Compress => {
                state
                    == Some(match strength {
                        Strength::Fast => FILE_PROVIDER_COMPRESSION_XPRESS4K,
                        Strength::Balanced => FILE_PROVIDER_COMPRESSION_XPRESS8K,
                        Strength::Maximum => FILE_PROVIDER_COMPRESSION_LZX,
                    })
            }
            Operation::Decompress => state.is_none(),
        };
        if operation == Operation::Decompress && !changed {
            return Err("Windows did not remove compression.".into());
        }
        // NTFS may trim provisional allocation when the final pre-operation
        // handle closes. Reopen after releasing that handle for the result.
        drop(file);
        let measured = fs::File::open(&path).map_err(|e| e.to_string())?;
        let measured_metadata = measured.metadata().map_err(|e| e.to_string())?;
        if filesystem::identity(&measured).map_err(|e| e.to_string())? != planned.identity {
            return Err("File changed before its final size could be measured.".into());
        }
        let after = filesystem::allocated(&path, &measured_metadata, &measured)
            .map_err(|e| e.to_string())?;
        Ok(Outcome {
            before,
            after,
            changed,
        })
    }
}

#[cfg(windows)]
pub use platform::{apply, apply_using, ensure_game_stopped, free_space, prepare, running_in};

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn every_strength_roundtrips_without_changing_contents() {
        for strength in [Strength::Fast, Strength::Balanced, Strength::Maximum] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("fixture.pak");
            let original = b"Disposable strength fixture ".repeat(80000);
            fs::write(&path, &original).unwrap();
            let plan = prepare(root.path(), Operation::Compress, &AtomicBool::new(false)).unwrap();
            let result = apply_using(&plan.root, plan.operation, &plan.files[0], strength).unwrap();
            assert!(result.changed);
            assert!(result.after < result.before);
            assert_eq!(fs::read(&path).unwrap(), original);
            let undo =
                prepare(root.path(), Operation::Decompress, &AtomicBool::new(false)).unwrap();
            assert_eq!(undo.files.len(), 1);
            apply_using(&undo.root, undo.operation, &undo.files[0], strength).unwrap();
            assert_eq!(fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn running_game_blocks_preparation_and_a_previously_prepared_file() {
        use std::{
            os::windows::process::CommandExt,
            process::{Child, Command},
            time::Duration,
        };
        struct OwnedProcess(Child);
        impl Drop for OwnedProcess {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data.pak");
        let original = b"GameZip fixture ".repeat(100000);
        fs::write(&data, &original).unwrap();
        let executable = root.path().join("fixture-game.exe");
        let source =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/ping.exe");
        fs::copy(source, &executable).unwrap();
        let plan = prepare(root.path(), Operation::Compress, &AtomicBool::new(false)).unwrap();
        let file = plan
            .files
            .iter()
            .find(|file| file.relative == Path::new("data.pak"))
            .unwrap();
        let _process = OwnedProcess(
            Command::new(&executable)
                .args(["-n", "30", "127.0.0.1"])
                .creation_flags(0x08000000)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        for _ in 0..50 {
            if !running_in(root.path()).unwrap().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!running_in(root.path()).unwrap().is_empty());
        assert!(
            prepare(root.path(), Operation::Compress, &AtomicBool::new(false))
                .unwrap_err()
                .contains("Close the game")
        );
        assert!(
            apply(&plan.root, plan.operation, file)
                .unwrap_err()
                .contains("Close the game")
        );
        assert_eq!(fs::read(&data).unwrap(), original);
    }

    #[test]
    fn locked_file_and_escaping_path_are_rejected_without_mutation() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("data.pak");
        let original = b"Disposable data ".repeat(100000);
        fs::write(&path, &original).unwrap();
        let plan = prepare(root.path(), Operation::Compress, &AtomicBool::new(false)).unwrap();
        let mut escaped = plan.files[0].clone();
        escaped.relative = PathBuf::from("../outside.pak");
        assert!(
            apply(&plan.root, plan.operation, &escaped)
                .unwrap_err()
                .contains("Invalid file path")
        );
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        assert!(apply(&plan.root, plan.operation, &plan.files[0]).is_err());
        drop(lock);
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn guarded_roundtrip_rejects_changed_and_shared_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("fixture [1] with spaces.pak");
        let original = b"GameZip test contents ".repeat(100000);
        fs::write(&path, &original).unwrap();
        let plan = prepare(root.path(), Operation::Compress, &AtomicBool::new(false)).unwrap();
        assert_eq!(plan.files.len(), 1);
        let result = apply(&plan.root, plan.operation, &plan.files[0]).unwrap();
        assert!(result.changed);
        assert!(result.after < result.before);
        assert_eq!(fs::read(&path).unwrap(), original);
        let undo = prepare(root.path(), Operation::Decompress, &AtomicBool::new(false)).unwrap();
        assert_eq!(undo.files.len(), 1);
        let restored = apply(&undo.root, undo.operation, &undo.files[0]).unwrap();
        assert!(restored.changed);
        assert_eq!(restored.before, result.after);
        let fresh = crate::scanner::scan(root.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(restored.after, fresh.allocated_bytes);
        assert!(restored.after >= original.len() as u64);
        assert_eq!(fs::read(&path).unwrap(), original);
        let stale = prepare(root.path(), Operation::Compress, &AtomicBool::new(false)).unwrap();
        fs::write(&path, b"Updated game file").unwrap();
        assert!(
            apply(&stale.root, stale.operation, &stale.files[0])
                .unwrap_err()
                .contains("changed")
        );
        assert_eq!(fs::read(&path).unwrap(), b"Updated game file");
        fs::hard_link(&path, root.path().join("shared.pak")).unwrap();
        let shared = prepare(root.path(), Operation::Compress, &AtomicBool::new(false)).unwrap();
        assert!(shared.files.is_empty());
        assert_eq!(shared.skipped, 2);
    }
}
