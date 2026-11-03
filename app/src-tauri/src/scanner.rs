use crate::{
    filesystem,
    model::{Category, LargeFile, ScanReport, now},
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub fn category(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "pak" | "archive" | "vpk" | "ba2" | "bsa" | "zip" | "7z" | "rar" | "pck" | "bundle"
        | "ucas" | "utoc" => "Archives",
        "mp4" | "webm" | "mkv" | "avi" | "mov" | "bik" | "bk2" | "wmv" | "usm" => "Videos",
        "wav" | "ogg" | "mp3" | "flac" | "wem" | "bnk" | "opus" => "Audio",
        "png" | "jpg" | "jpeg" | "dds" | "tga" | "bmp" | "webp" | "ktx" => "Images",
        "exe" | "dll" | "so" | "sys" => "Programs",
        _ => "Other",
    }
}

fn issue(report: &mut ScanReport, path: &Path, reason: impl std::fmt::Display) {
    report.skipped += 1;
    if report.issues.len() < 20 {
        report
            .issues
            .push(format!("{}: {reason}", filesystem::display(path)));
    }
}

/// Read-only traversal. Never follows symlinks/junctions; memory is bounded for file results.
pub fn scan(
    root: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<ScanReport, String> {
    if !root.is_dir() {
        return Err("The game folder is unavailable. Reconnect the drive and try again.".into());
    }
    let mut report = ScanReport::default();
    let mut directories = vec![root.to_path_buf()];
    let mut seen = HashSet::new();
    let mut categories: BTreeMap<String, Category> = BTreeMap::new();
    let mut tick = Instant::now();
    while let Some(directory) = directories.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(e) => {
                issue(&mut report, &directory, e);
                continue;
            }
        };
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    issue(&mut report, &directory, e);
                    continue;
                }
            };
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    issue(&mut report, &path, e);
                    continue;
                }
            };
            if filesystem::is_link(&metadata) {
                issue(&mut report, &path, "Link or junction skipped");
                continue;
            }
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let file = match fs::File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    issue(&mut report, &path, e);
                    continue;
                }
            };
            let identity = match filesystem::identity(&file) {
                Ok(id) => id,
                Err(e) => {
                    issue(&mut report, &path, e);
                    continue;
                }
            };
            if !seen.insert(identity) {
                report.duplicate_links += 1;
                continue;
            }
            let disk = filesystem::allocated(&path, &metadata, &file).ok();
            if disk.is_none() {
                report.allocation_unknown += 1;
            }
            let kind = category(&path);
            let group = categories.entry(kind.into()).or_insert_with(|| Category {
                name: kind.into(),
                files: 0,
                logical_bytes: 0,
                allocated_bytes: 0,
            });
            group.files += 1;
            group.logical_bytes += metadata.len();
            group.allocated_bytes += disk.unwrap_or(0);
            report.files += 1;
            report.logical_bytes += metadata.len();
            report.allocated_bytes += disk.unwrap_or(0);
            report.largest_files.push(LargeFile {
                path: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into(),
                category: kind.into(),
                logical_bytes: metadata.len(),
                allocated_bytes: disk,
            });
            if report.largest_files.len() > 200 {
                report
                    .largest_files
                    .sort_by_key(|f| std::cmp::Reverse(f.logical_bytes));
                report.largest_files.truncate(100);
            }
            if tick.elapsed() >= Duration::from_millis(250) {
                progress(report.files, report.logical_bytes);
                tick = Instant::now();
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    report
        .largest_files
        .sort_by_key(|f| std::cmp::Reverse(f.logical_bytes));
    report.largest_files.truncate(100);
    report.categories = categories.into_values().collect();
    report.completed_at = now();
    progress(report.files, report.logical_bytes);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_unique_files_and_classifies_opaque_archives_without_guessing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("textures.pak"), vec![1; 12000]).unwrap();
        fs::write(dir.path().join("nested/intro.mp4"), vec![2; 5000]).unwrap();
        fs::hard_link(dir.path().join("textures.pak"), dir.path().join("copy.pak")).unwrap();
        let report = scan(dir.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(report.files, 2);
        assert_eq!(report.logical_bytes, 17000);
        assert_eq!(report.duplicate_links, 1);
        assert_eq!(
            report
                .categories
                .iter()
                .find(|c| c.name == "Archives")
                .unwrap()
                .logical_bytes,
            12000
        );
        assert_eq!(report.largest_files[0].logical_bytes, 12000);
        assert_eq!(report.allocation_unknown, 0);
    }
    #[test]
    fn cancellation_does_not_publish_partial_results() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            scan(dir.path(), &AtomicBool::new(true), |_, _| {}).unwrap_err(),
            "cancelled"
        );
    }
    #[test]
    fn empty_folder_is_a_valid_zero_result() {
        let dir = tempfile::tempdir().unwrap();
        let report = scan(dir.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(report.files, 0);
        assert!(report.categories.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn ordinary_file_disk_usage_includes_cluster_slack() {
        let dir = tempfile::tempdir().unwrap();
        // Larger than an NTFS resident record, but not a multiple of a sector.
        fs::write(dir.path().join("unaligned.dat"), vec![1; 5001]).unwrap();
        let report = scan(dir.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(report.logical_bytes, 5001);
        assert_eq!(report.allocation_unknown, 0);
        assert!(report.allocated_bytes > report.logical_bytes);
        assert_eq!(report.allocated_bytes % 512, 0);
    }

    #[cfg(windows)]
    #[test]
    fn locked_file_is_reported_instead_of_being_counted_as_zero() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.pak");
        fs::write(&path, vec![1; 4096]).unwrap();
        let _lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let report = scan(dir.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(report.files, 0);
        assert_eq!(report.skipped, 1);
        assert_eq!(report.issues.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_not_followed_outside_the_game_root() {
        use std::process::{Command, Stdio};
        let game = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.dat"), vec![1; 10000]).unwrap();
        let link = game.path().join("junction");
        assert!(
            Command::new("cmd.exe")
                .args(["/c", "mklink", "/J"])
                .arg(&link)
                .arg(outside.path())
                .stdout(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        let report = scan(game.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
        assert_eq!(report.files, 0);
        assert_eq!(report.skipped, 1);
        // Remove only the junction itself; the target remains independently owned.
        fs::remove_dir(&link).unwrap();
        assert!(outside.path().join("outside.dat").exists());
    }
}
