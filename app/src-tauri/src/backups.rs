//! Local original-copy vault for the video pipeline. No method in this module
//! replaces game files. Only verified, completed backups can expire; recovery
//! and restore-in-progress records are retained across restarts.
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const DAY: u64 = 86_400;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum State {
    Preparing,
    Ready,
    Completed,
    Restoring,
    Deleting,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backup {
    pub id: String,
    pub original_path: PathBuf,
    pub original_hash: String,
    pub converted_hash: Option<String>,
    pub bytes: u64,
    pub retention_days: u32,
    pub expires_at: Option<u64>,
    pub state: State,
}

pub struct Vault {
    root: PathBuf,
    connection: Mutex<Connection>,
    // Keep directory ancestors from being renamed while path-based operations
    // run. File handles independently guard each blob against replacement.
    _directories: Vec<File>,
}

fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub(crate) fn pin_directories(path: &Path) -> Result<Vec<File>, String> {
    let mut handles = Vec::new();
    for ancestor in path.ancestors() {
        let meta = fs::symlink_metadata(ancestor).map_err(error)?;
        if !meta.is_dir() || crate::filesystem::is_link(&meta) {
            return Err("Backup paths must not contain links or junctions.".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::*;
            let handle = OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(ancestor)
                .map_err(error)?;
            let opened = handle.metadata().map_err(error)?;
            if !opened.is_dir() || crate::filesystem::is_link(&opened) {
                return Err("Directory changed or contains a junction.".into());
            }
            handles.push(handle);
        }
    }
    Ok(handles)
}

pub(crate) fn read_locked(path: &Path, deleting: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::*;
        // DELETE is an access right; using the opened handle avoids deleting a
        // replacement that appeared at the same path after verification.
        options
            .access_mode(FILE_GENERIC_READ | if deleting { 0x00010000 } else { 0 })
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path).map_err(error)?;
    let meta = file.metadata().map_err(error)?;
    if !meta.is_file() || crate::filesystem::is_link(&meta) {
        return Err("Backup source must be a regular file.".into());
    }
    Ok(file)
}

fn digest(file: &mut File) -> Result<String, String> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(error)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn write_record(db: &Connection, backup: &Backup) -> Result<(), String> {
    db.execute("INSERT INTO backups(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![backup.id, serde_json::to_string(backup).map_err(error)?]).map_err(error)?;
    Ok(())
}
fn record(db: &Connection, id: &str) -> Result<Backup, String> {
    let json: String = db
        .query_row("SELECT payload FROM backups WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .map_err(error)?;
    serde_json::from_str(&json).map_err(error)
}

impl Vault {
    /// The caller selects an existing, dedicated backup directory. The ledger
    /// belongs inside that directory so reconnecting a drive retains its records.
    pub fn open(root: &Path) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err("Choose an absolute backup directory.".into());
        }
        let directories = pin_directories(root)?;
        let root = fs::canonicalize(root).map_err(error)?;
        let db = Connection::open(root.join("gamezip-backups.sqlite")).map_err(error)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS backups(id TEXT PRIMARY KEY,payload TEXT NOT NULL);").map_err(error)?;
        Ok(Self {
            root,
            connection: Mutex::new(db),
            _directories: directories,
        })
    }

    fn blob(&self, id: &str) -> Result<PathBuf, String> {
        let id = uuid::Uuid::parse_str(id).map_err(error)?;
        Ok(self.root.join(format!("{id}.original")))
    }

    pub fn list(&self) -> Result<Vec<Backup>, String> {
        let db = self.connection.lock().map_err(error)?;
        let mut query = db
            .prepare("SELECT payload FROM backups ORDER BY rowid DESC")
            .map_err(error)?;
        query
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(error)?
            .map(|row| serde_json::from_str(&row.map_err(error)?).map_err(error))
            .collect()
    }

    pub fn preserve(&self, source: &Path, retention_days: u32) -> Result<Backup, String> {
        if retention_days < 7 {
            return Err("Originals must be retained for at least seven days.".into());
        }
        let _source_dirs = pin_directories(source.parent().ok_or("Missing source directory")?)?;
        let mut input = read_locked(source, false)?;
        let mut db = self.connection.lock().map_err(error)?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        let mut backup = Backup {
            id: uuid::Uuid::new_v4().to_string(),
            original_path: fs::canonicalize(source).map_err(error)?,
            original_hash: String::new(),
            converted_hash: None,
            bytes: input.metadata().map_err(error)?.len(),
            retention_days,
            expires_at: None,
            state: State::Preparing,
        };
        write_record(&tx, &backup)?;
        tx.commit().map_err(error)?;
        // A failed copy leaves a Preparing record and recovery bytes. Cleanup
        // never expires these records, including after a process crash.
        let path = self.blob(&backup.id)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(error)?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer).map_err(error)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count]).map_err(error)?;
            hash.update(&buffer[..count]);
        }
        output.sync_all().map_err(error)?;
        drop(output);
        backup.original_hash = format!("{:x}", hash.finalize());
        if digest(&mut read_locked(&path, false)?)? != backup.original_hash {
            return Err("The original backup failed verification.".into());
        }
        backup.state = State::Ready;
        write_record(&db, &backup)?;
        Ok(backup)
    }

    /// Call only after the pipeline has validated playback metadata and replaced
    /// the source successfully. Recheck the exact converted bytes before starting
    /// retention. Repeated calls cannot reset or shorten the retention clock.
    pub fn complete(&self, id: &str, converted_hash: &str, now: u64) -> Result<Backup, String> {
        let mut db = self.connection.lock().map_err(error)?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        let mut backup = record(&tx, id)?;
        if backup.state != State::Ready {
            return Err("Backup is not awaiting a successful conversion.".into());
        }
        let _dirs = pin_directories(
            backup
                .original_path
                .parent()
                .ok_or("Missing original directory")?,
        )?;
        if digest(&mut read_locked(&backup.original_path, false)?)? != converted_hash {
            return Err("Converted file changed. Recovery copy retained.".into());
        }
        if digest(&mut read_locked(&self.blob(id)?, false)?)? != backup.original_hash {
            return Err("Original backup verification failed.".into());
        }
        backup.expires_at = Some(
            now.checked_add(u64::from(backup.retention_days) * DAY)
                .ok_or("Retention date overflow")?,
        );
        backup.converted_hash = Some(converted_hash.into());
        backup.state = State::Completed;
        write_record(&tx, &backup)?;
        tx.commit().map_err(error)?;
        Ok(backup)
    }

    /// Export is the safe resolution for a game-update conflict: it creates a
    /// separate file and never overwrites either the game or an existing export.
    pub fn export(&self, id: &str, destination: &Path) -> Result<(), String> {
        let db = self.connection.lock().map_err(error)?;
        let backup = record(&db, id)?;
        if !matches!(
            backup.state,
            State::Ready | State::Completed | State::Restoring
        ) {
            return Err("This backup is not available for export.".into());
        }
        let _dirs = pin_directories(destination.parent().ok_or("Missing export directory")?)?;
        let mut source = read_locked(&self.blob(id)?, false)?;
        if digest(&mut source)? != backup.original_hash {
            return Err("Original backup verification failed.".into());
        }
        use std::io::{Seek, SeekFrom};
        source.seek(SeekFrom::Start(0)).map_err(error)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(error)?;
        std::io::copy(&mut source, &mut output).map_err(error)?;
        output.sync_all().map_err(error)
    }

    pub fn begin_restore(&self, id: &str) -> Result<Backup, String> {
        let mut db = self.connection.lock().map_err(error)?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(error)?;
        let mut backup = record(&tx, id)?;
        if backup.state != State::Completed {
            return Err("Backup is not ready for restoration.".into());
        }
        let _dirs = pin_directories(
            backup
                .original_path
                .parent()
                .ok_or("Missing original directory")?,
        )?;
        if Some(digest(&mut read_locked(&backup.original_path, false)?)?) != backup.converted_hash {
            return Err(
                "The game file changed. Preserve it and export the backup separately.".into(),
            );
        }
        backup.state = State::Restoring;
        write_record(&tx, &backup)?;
        tx.commit().map_err(error)?;
        Ok(backup)
    }

    pub fn expire(&self, now: u64) -> Result<usize, String> {
        let candidates = self.list()?;
        let mut count = 0;
        for candidate in candidates {
            let mut db = self.connection.lock().map_err(error)?;
            let tx = db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(error)?;
            let mut backup = record(&tx, &candidate.id)?;
            if !matches!(backup.state, State::Completed | State::Deleting)
                || !backup.expires_at.is_some_and(|expires| now >= expires)
            {
                continue;
            }
            let path = self.blob(&backup.id)?;
            // Recover a crash after deleting the bytes but before updating the ledger.
            if backup.state == State::Deleting && !path.try_exists().map_err(error)? {
                backup.state = State::Expired;
                write_record(&tx, &backup)?;
                tx.commit().map_err(error)?;
                count += 1;
                continue;
            }
            let mut file = read_locked(&path, true)?;
            if digest(&mut file)? != backup.original_hash {
                return Err("Expired backup changed; retained for review.".into());
            }
            backup.state = State::Deleting;
            write_record(&tx, &backup)?;
            tx.commit().map_err(error)?;
            #[cfg(windows)]
            {
                use std::os::windows::io::AsRawHandle;
                use windows_sys::Win32::Storage::FileSystem::{
                    FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
                };
                let info = FILE_DISPOSITION_INFO { DeleteFile: true };
                // Mark only the verified, locked file for deletion when closed.
                if unsafe {
                    SetFileInformationByHandle(
                        file.as_raw_handle(),
                        FileDispositionInfo,
                        (&info as *const FILE_DISPOSITION_INFO).cast(),
                        std::mem::size_of_val(&info) as u32,
                    )
                } == 0
                {
                    return Err(error(std::io::Error::last_os_error()));
                }
            }
            #[cfg(not(windows))]
            {
                return Err("Backup expiry is supported on Windows only.".into());
            }
            drop(file);
            backup.state = State::Expired;
            write_record(&db, &backup)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn retention_starts_only_after_success_and_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("vault");
        fs::create_dir(&root).unwrap();
        let source = temp.path().join("video.mp4");
        fs::write(&source, b"original").unwrap();
        let vault = Vault::open(&root).unwrap();
        assert!(vault.preserve(&source, 6).is_err());
        let backup = vault.preserve(&source, 7).unwrap();
        assert_eq!(backup.state, State::Ready);
        assert_eq!(backup.expires_at, None);
        assert_eq!(vault.expire(u64::MAX).unwrap(), 0);
        assert!(
            vault
                .complete(&backup.id, &hash(b"converted"), 100)
                .is_err()
        );
        fs::write(&source, b"converted").unwrap();
        let completed = vault
            .complete(&backup.id, &hash(b"converted"), 100)
            .unwrap();
        assert_eq!(completed.expires_at, Some(100 + 7 * DAY));
        assert!(vault.complete(&backup.id, &hash(b"converted"), 0).is_err());
        drop(vault);
        let vault = Vault::open(&root).unwrap();
        assert_eq!(vault.expire(100 + 7 * DAY - 1).unwrap(), 0);
        assert_eq!(vault.expire(100 + 7 * DAY).unwrap(), 1);
        assert_eq!(vault.list().unwrap()[0].state, State::Expired);
        assert!(!vault.blob(&backup.id).unwrap().exists());
        assert_eq!(fs::read(&source).unwrap(), b"converted");
    }

    #[test]
    fn game_update_is_preserved_and_verified_backup_exports_separately() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("vault");
        fs::create_dir(&root).unwrap();
        let source = temp.path().join("video.mp4");
        fs::write(&source, b"original").unwrap();
        let vault = Vault::open(&root).unwrap();
        let backup = vault.preserve(&source, 7).unwrap();
        fs::write(&source, b"converted").unwrap();
        vault
            .complete(&backup.id, &hash(b"converted"), 100)
            .unwrap();
        fs::write(&source, b"new game update").unwrap();
        assert!(
            vault
                .begin_restore(&backup.id)
                .unwrap_err()
                .contains("changed")
        );
        assert!(vault.export(&backup.id, &source).is_err());
        let exported = temp.path().join("export.mp4");
        vault.export(&backup.id, &exported).unwrap();
        assert_eq!(fs::read(&exported).unwrap(), b"original");
        assert_eq!(fs::read(&source).unwrap(), b"new game update");
    }

    #[test]
    fn incomplete_restore_and_corrupt_originals_are_never_expired() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("vault");
        fs::create_dir(&root).unwrap();
        let source = temp.path().join("video.mp4");
        fs::write(&source, b"original").unwrap();
        let vault = Vault::open(&root).unwrap();
        let backup = vault.preserve(&source, 10).unwrap();
        fs::write(&source, b"converted").unwrap();
        vault
            .complete(&backup.id, &hash(b"converted"), 100)
            .unwrap();
        vault.begin_restore(&backup.id).unwrap();
        drop(vault);
        let vault = Vault::open(&root).unwrap();
        assert_eq!(vault.expire(u64::MAX).unwrap(), 0);
        let second = vault.preserve(&source, 7).unwrap();
        vault
            .complete(&second.id, &hash(b"converted"), 100)
            .unwrap();
        fs::write(vault.blob(&second.id).unwrap(), b"damaged").unwrap();
        assert!(vault.expire(u64::MAX).is_err());
        assert!(vault.blob(&second.id).unwrap().exists());
        assert!(
            vault
                .export(&second.id, &temp.path().join("export.mp4"))
                .is_err()
        );
    }

    #[test]
    fn interrupted_copy_is_retained_and_interrupted_deletion_is_reconciled() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("vault");
        fs::create_dir(&root).unwrap();
        let source = temp.path().join("video.mp4");
        fs::write(&source, b"original").unwrap();
        let vault = Vault::open(&root).unwrap();
        let mut partial = vault.preserve(&source, 7).unwrap();
        partial.state = State::Preparing;
        write_record(&vault.connection.lock().unwrap(), &partial).unwrap();
        let mut deleting = vault.preserve(&source, 7).unwrap();
        deleting.state = State::Deleting;
        deleting.expires_at = Some(1);
        write_record(&vault.connection.lock().unwrap(), &deleting).unwrap();
        fs::remove_file(vault.blob(&deleting.id).unwrap()).unwrap();
        drop(vault);
        let vault = Vault::open(&root).unwrap();
        assert_eq!(vault.expire(100).unwrap(), 1);
        assert!(vault.blob(&partial.id).unwrap().exists());
        assert_eq!(
            vault
                .list()
                .unwrap()
                .iter()
                .find(|b| b.id == deleting.id)
                .unwrap()
                .state,
            State::Expired
        );
    }
}
