use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub name: String,
    pub files: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFile {
    pub path: String,
    pub category: String,
    pub logical_bytes: u64,
    pub allocated_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub files: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub allocation_unknown: u64,
    pub skipped: u64,
    pub duplicate_links: u64,
    pub issues: Vec<String>,
    pub categories: Vec<Category>,
    pub largest_files: Vec<LargeFile>,
    pub completed_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub path: String,
    pub launcher: String,
    pub missing: bool,
    pub scan: Option<ScanReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub game_id: String,
    pub game_name: String,
    pub state: String,
    pub files: u64,
    pub logical_bytes: u64,
    pub message: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub name: String,
    pub path: String,
    pub launcher: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Discovery {
    pub games: Vec<Candidate>,
    pub warnings: Vec<String>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
