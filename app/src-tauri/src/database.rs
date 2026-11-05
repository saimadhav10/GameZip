use crate::{
    filesystem,
    model::{Game, Job, ScanReport},
};
use rusqlite::{Connection, params};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Database {
    connection: Mutex<Connection>,
    // Preserve terminal state for this session if SQLite cannot write it.
    // Otherwise polling would keep reporting a completed worker as running.
    unsaved_jobs: Mutex<Vec<Job>>,
    pub path: PathBuf,
}

impl Database {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let connection = Connection::open(&path).map_err(|e| e.to_string())?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS games(id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE, launcher TEXT NOT NULL, scan TEXT);
            CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY, payload TEXT NOT NULL);")
            .map_err(|e| e.to_string())?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS compression_runs(job_id TEXT PRIMARY KEY, plan TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS file_steps(job_id TEXT NOT NULL, ordinal INTEGER NOT NULL, state TEXT NOT NULL, before_bytes INTEGER, after_bytes INTEGER, message TEXT, PRIMARY KEY(job_id,ordinal));")
            .map_err(|e| e.to_string())?;
        let db = Self {
            connection: Mutex::new(connection),
            unsaved_jobs: Mutex::new(Vec::new()),
            path,
        };
        for mut job in db.jobs()? {
            if job.state == "running" || job.state == "stopping" {
                job.state = "interrupted".into();
                job.message = if job.operation == "scan" {
                    "GameZip closed before this scan finished. Run a new scan to refresh its results."
                } else {
                    "GameZip closed before this job finished. Analyse the game, then prepare the operation again; current file states will be checked."
                }.into();
                job.finished_at = Some(crate::model::now());
                db.save_job(&job)?;
            }
        }
        Ok(db)
    }

    pub fn games(&self) -> Result<Vec<Game>, String> {
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let mut statement = connection
            .prepare("SELECT id,name,path,launcher,scan FROM games ORDER BY name")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut games = Vec::new();
        for row in rows {
            let (id, name, path, launcher, json) = row.map_err(|e| e.to_string())?;
            let scan = json
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(|e| format!("Cannot read saved scan: {e}"))?;
            games.push(Game {
                id,
                name,
                missing: !Path::new(&path).is_dir(),
                path,
                launcher,
                scan,
            });
        }
        games.sort_by_key(|g| std::cmp::Reverse(g.scan.as_ref().map_or(0, |s| s.allocated_bytes)));
        Ok(games)
    }

    pub fn game(&self, id: &str) -> Result<Game, String> {
        self.games()?
            .into_iter()
            .find(|g| g.id == id)
            .ok_or_else(|| "Game is no longer in your library.".into())
    }

    pub fn add(&self, name: &str, path: &str, launcher: &str) -> Result<Game, String> {
        let canonical = filesystem::root(path)?;
        // Serialize overlap checking and insertion to avoid duplicate concurrent imports.
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let mut query = connection
            .prepare("SELECT id,name,path,launcher,scan FROM games")
            .map_err(|e| e.to_string())?;
        let records = query
            .query_map([], |row| row.get::<_, String>(2))
            .map_err(|e| e.to_string())?;
        for record in records {
            let saved = PathBuf::from(record.map_err(|e| e.to_string())?);
            let existing = std::fs::canonicalize(&saved).unwrap_or(saved);
            if roots_overlap(&canonical, &existing) {
                return Err(
                    "This folder is already in your library, or overlaps an existing game folder."
                        .into(),
                );
            }
        }
        drop(query);
        let fallback = canonical.file_name().unwrap_or_default().to_string_lossy();
        let title = if name.trim().is_empty() {
            fallback.to_string()
        } else {
            name.trim().chars().take(200).collect()
        };
        let game = Game {
            id: uuid::Uuid::new_v4().to_string(),
            name: title,
            path: filesystem::display(&canonical),
            launcher: launcher.into(),
            missing: false,
            scan: None,
        };
        connection
            .execute(
                "INSERT INTO games(id,name,path,launcher) VALUES (?1,?2,?3,?4)",
                params![game.id, game.name, game.path, game.launcher],
            )
            .map_err(|e| e.to_string())?;
        Ok(game)
    }

    pub fn save_scan(&self, id: &str, scan: &ScanReport) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let json = serde_json::to_string(scan).map_err(|e| e.to_string())?;
        let changed = connection
            .execute("UPDATE games SET scan=?1 WHERE id=?2", params![json, id])
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("The game was removed while scanning.".into());
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        self.connection
            .lock()
            .map_err(|e| e.to_string())?
            .execute("DELETE FROM games WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_job(&self, job: &Job) -> Result<(), String> {
        let json = serde_json::to_string(job).map_err(|e| e.to_string())?;
        self.connection.lock().map_err(|e| e.to_string())?.execute("INSERT INTO jobs(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![job.id,json]).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn begin_compression(
        &self,
        job: &Job,
        plan: &crate::compression::Plan,
    ) -> Result<(), String> {
        let mut connection = self.connection.lock().map_err(|e| e.to_string())?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction
            .execute(
                "INSERT INTO jobs(id,payload) VALUES(?1,?2)",
                params![
                    job.id,
                    serde_json::to_string(job).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        transaction
            .execute(
                "INSERT INTO compression_runs(job_id,plan) VALUES(?1,?2)",
                params![
                    job.id,
                    serde_json::to_string(plan).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO file_steps(job_id,ordinal,state) VALUES(?1,?2,'pending')")
                .map_err(|e| e.to_string())?;
            for index in 0..plan.files.len() {
                insert
                    .execute(params![job.id, index as i64])
                    .map_err(|e| e.to_string())?;
            }
        }
        // Never display a pre-modification scan as a current result after a crash.
        transaction
            .execute("UPDATE games SET scan=NULL WHERE id=?1", [&job.game_id])
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    pub fn file_step(
        &self,
        job_id: &str,
        index: usize,
        state: &str,
        before: Option<u64>,
        after: Option<u64>,
        message: &str,
    ) -> Result<(), String> {
        let before = before
            .map(i64::try_from)
            .transpose()
            .map_err(|e| e.to_string())?;
        let after = after
            .map(i64::try_from)
            .transpose()
            .map_err(|e| e.to_string())?;
        let changed = self.connection.lock().map_err(|e| e.to_string())?.execute(
            "UPDATE file_steps SET state=?1,before_bytes=?2,after_bytes=?3,message=?4 WHERE job_id=?5 AND ordinal=?6",
            params![state,before,after,message,job_id,index as i64],
        ).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err(
                "Cannot find the file's recovery record. No further files will be changed.".into(),
            );
        }
        Ok(())
    }

    pub fn finish_job(&self, mut job: Job) -> Job {
        if let Err(error) = self.save_job(&job) {
            job.state = "failed".into();
            job.message = format!("Could not save job result: {error}");
            let mut unsaved = self.unsaved_jobs.lock().unwrap_or_else(|e| e.into_inner());
            unsaved.retain(|previous| previous.id != job.id);
            unsaved.push(job.clone());
            if unsaved.len() > 100 {
                unsaved.remove(0);
            }
        }
        job
    }

    pub fn jobs(&self) -> Result<Vec<Job>, String> {
        let connection = self.connection.lock().map_err(|e| e.to_string())?;
        let mut query = connection
            .prepare("SELECT payload FROM jobs ORDER BY rowid DESC LIMIT 100")
            .map_err(|e| e.to_string())?;
        let records = query
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut jobs: Vec<Job> = records
            .map(|row| {
                let json = row.map_err(|e| e.to_string())?;
                serde_json::from_str(&json).map_err(|e| e.to_string())
            })
            .collect::<Result<_, String>>()?;
        for job in self
            .unsaved_jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            if let Some(saved) = jobs.iter_mut().find(|saved| saved.id == job.id) {
                *saved = job.clone();
            } else {
                jobs.insert(0, job.clone());
            }
        }
        jobs.truncate(100);
        Ok(jobs)
    }
}

fn roots_overlap(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    let (a, b) = (
        PathBuf::from(a.to_string_lossy().to_lowercase()),
        PathBuf::from(b.to_string_lossy().to_lowercase()),
    );
    a.starts_with(&b) || b.starts_with(&a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_file_journal_write_stops_before_compression() {
        use crate::{
            compression::{self, Operation},
            compression_job,
        };
        use std::sync::atomic::AtomicBool;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        std::fs::create_dir(&root).unwrap();
        let contents = b"GameZip journal fixture ".repeat(100000);
        std::fs::write(root.join("data.pak"), &contents).unwrap();
        let db_path = temp.path().join("jobs.sqlite");
        let db = Database::open(db_path.clone()).unwrap();
        let game = db.add("Fixture", root.to_str().unwrap(), "Manual").unwrap();
        let cancel = AtomicBool::new(false);
        let plan = compression::prepare(&root, Operation::Compress, &cancel).unwrap();
        let job = Job {
            id: "journal-fixture".into(),
            game_id: game.id,
            game_name: game.name,
            operation: "compress".into(),
            state: "running".into(),
            total_files: Some(1),
            ..Default::default()
        };
        db.begin_compression(&job, &plan).unwrap();
        db.connection
            .lock()
            .unwrap()
            .execute_batch("PRAGMA query_only=ON")
            .unwrap();
        let finished = compression_job::run(&db, &plan, job, &cancel, |_| {});
        assert_eq!(finished.state, "failed");
        assert_eq!(finished.files, 0);
        assert_eq!(std::fs::read(root.join("data.pak")).unwrap(), contents);
        let still_plain = compression::prepare(&root, Operation::Compress, &cancel).unwrap();
        assert_eq!(still_plain.files.len(), 1);
        drop(db);
        let recovered = Database::open(db_path).unwrap();
        assert_eq!(recovered.jobs().unwrap()[0].state, "interrupted");
        let state: String = recovered
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT state FROM file_steps WHERE job_id='journal-fixture'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "pending");
    }
    #[test]
    fn library_persists_rejects_overlaps_and_removal_preserves_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/file.pak"), b"original").unwrap();
        let path = temp.path().join("library.sqlite");
        let db = Database::open(path.clone()).unwrap();
        let game = db.add("Example", root.to_str().unwrap(), "Manual").unwrap();
        assert!(
            db.add("Nested", root.join("data").to_str().unwrap(), "Manual")
                .is_err()
        );
        drop(db);
        let db = Database::open(path).unwrap();
        assert_eq!(db.games().unwrap()[0].id, game.id);
        db.remove(&game.id).unwrap();
        assert!(root.join("data/file.pak").exists());
        assert!(db.games().unwrap().is_empty());
    }
    #[test]
    fn failed_final_write_reports_terminal_state_instead_of_a_phantom_active_job() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite");
        let db = Database::open(path.clone()).unwrap();
        let mut job = Job {
            id: "job".into(),
            game_id: "game".into(),
            game_name: "Test".into(),
            state: "running".into(),
            files: 2,
            logical_bytes: 100,
            message: "Scanning".into(),
            started_at: 1,
            finished_at: None,
            operation: "scan".into(),
            ..Default::default()
        };
        db.save_job(&job).unwrap();
        db.connection
            .lock()
            .unwrap()
            .execute_batch("PRAGMA query_only=ON")
            .unwrap();
        job.state = "completed".into();
        job.finished_at = Some(2);
        let finished = db.finish_job(job);
        assert_eq!(finished.state, "failed");
        let visible = db.jobs().unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].state, "failed");
        assert!(visible[0].message.contains("Could not save job result"));
        assert_eq!(visible[0].finished_at, Some(2));
        drop(db);
        // Restart still reconciles the durable unfinished record honestly.
        assert_eq!(
            Database::open(path).unwrap().jobs().unwrap()[0].state,
            "interrupted"
        );
    }

    #[test]
    fn reopening_marks_unfinished_jobs_interrupted() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("library.sqlite");
        let db = Database::open(path.clone()).unwrap();
        db.save_job(&Job {
            id: "job".into(),
            game_id: "game".into(),
            game_name: "Test".into(),
            state: "running".into(),
            files: 2,
            logical_bytes: 100,
            message: "Scanning".into(),
            started_at: 1,
            finished_at: None,
            operation: "scan".into(),
            ..Default::default()
        })
        .unwrap();
        drop(db);
        assert_eq!(
            Database::open(path).unwrap().jobs().unwrap()[0].state,
            "interrupted"
        );
    }
}
