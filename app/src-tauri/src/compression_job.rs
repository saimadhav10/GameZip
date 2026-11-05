use crate::{
    compression::{self, Plan},
    database::Database,
    model::{Job, now},
    scanner,
};
use std::sync::atomic::{AtomicBool, Ordering};

/// Every mutation is bracketed by durable file records. A failed journal write
/// stops the worker; restart prepares against live compression state, never
/// blindly replays a file whose last recorded state was "running".
pub fn run(
    db: &Database,
    plan: &Plan,
    mut job: Job,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&Job),
) -> Job {
    let mut changed = 0;
    for (index, file) in plan.files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            job.state = "cancelled".into();
            job.message =
                "Stopped safely. Completed files keep their current compression state.".into();
            break;
        }
        job.message = file.relative.to_string_lossy().into();
        progress(&job);
        if let Err(error) = db.file_step(&job.id, index, "running", Some(file.allocated), None, "")
        {
            job.state = "failed".into();
            job.message = error;
            break;
        }
        match compression::apply(&plan.root, plan.operation, file) {
            Ok(result) => {
                if let Err(error) = db.file_step(
                    &job.id,
                    index,
                    if result.changed {
                        "completed"
                    } else {
                        "unchanged"
                    },
                    Some(result.before),
                    Some(result.after),
                    "",
                ) {
                    job.state = "failed".into();
                    job.message =
                        format!("Stopped because the recovery record could not be saved: {error}");
                    break;
                }
                job.files += 1;
                job.logical_bytes = job.logical_bytes.saturating_add(file.logical);
                job.before_bytes = job.before_bytes.saturating_add(result.before);
                job.after_bytes = job.after_bytes.saturating_add(result.after);
                changed += u64::from(result.changed);
                if let Err(error) = db.save_job(&job) {
                    job.state = "failed".into();
                    job.message = format!("Stopped because progress could not be saved: {error}");
                    break;
                }
                progress(&job);
            }
            Err(error) => {
                let saved =
                    db.file_step(&job.id, index, "failed", Some(file.allocated), None, &error);
                job.state = "failed".into();
                job.message = format!("{}: {error}", file.relative.display());
                if let Err(write_error) = saved {
                    job.message.push_str(&format!(
                        " Recovery record could not be saved: {write_error}"
                    ));
                }
                break;
            }
        }
    }
    if job.state == "running" {
        if cancel.load(Ordering::Relaxed) {
            job.state = "cancelled".into();
            job.message =
                "Stopped safely. Completed files keep their current compression state.".into();
        } else {
            job.message = "Refreshing disk usage".into();
            progress(&job);
            match scanner::scan(&plan.root, cancel, |_, _| {}) {
                Ok(report) => match db.save_scan(&job.game_id, &report) {
                    Ok(()) => {
                        job.state = "completed".into();
                        job.message = format!(
                            "Finished. {changed} files changed; {} left unchanged or excluded.",
                            plan.skipped + job.files - changed
                        );
                    }
                    Err(error) => {
                        job.state = "failed".into();
                        job.message = format!(
                            "Files processed, but the new scan could not be saved: {error}"
                        );
                    }
                },
                Err(error) => {
                    job.state = if error == "cancelled" {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into();
                    job.message =
                        "Files processed. Analyse the game again to refresh its disk usage.".into();
                }
            }
        }
    }
    job.finished_at = Some(now());
    db.finish_job(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn cancellation_keeps_completed_files_and_records_remaining_work() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fs::create_dir(&root).unwrap();
        let original = b"Disposable game fixture ".repeat(100000);
        for name in ["one.pak", "two.pak"] {
            fs::write(root.join(name), &original).unwrap();
        }
        let db = Database::open(temp.path().join("jobs.sqlite")).unwrap();
        let game = db.add("Fixture", root.to_str().unwrap(), "Manual").unwrap();
        let cancel = AtomicBool::new(false);
        let plan = compression::prepare(&root, compression::Operation::Compress, &cancel).unwrap();
        let job = Job {
            id: "fixture-job".into(),
            game_id: game.id.clone(),
            game_name: game.name,
            operation: "compress".into(),
            state: "running".into(),
            total_files: Some(2),
            started_at: now(),
            ..Default::default()
        };
        db.begin_compression(&job, &plan).unwrap();
        let finished = run(&db, &plan, job, &cancel, |job| {
            if job.files == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(finished.state, "cancelled");
        assert_eq!(finished.files, 1);
        assert!(finished.before_bytes > finished.after_bytes);
        assert!(db.game(&game.id).unwrap().scan.is_none());
        for name in ["one.pak", "two.pak"] {
            assert_eq!(fs::read(root.join(name)).unwrap(), original);
        }
        // New preparation checks actual state and includes only the remaining file.
        let next = compression::prepare(
            &root,
            compression::Operation::Compress,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(next.files.len(), 1);
    }
}
