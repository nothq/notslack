use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use fs2::FileExt;
use serde_json::{Map, Value};

const RUNTIME_STATE_FILE: &str = "runtime-state.json";
const RUNTIME_STATE_LOCK_FILE: &str = "runtime-state.json.lock";

pub fn load_runtime_state_json(runtime_root: &Path) -> Result<Value, String> {
    load_runtime_state_from_path(&runtime_state_path(runtime_root))
}

pub fn mutate_runtime_state_json(
    runtime_root: &Path,
    mutation: impl FnOnce(&mut Map<String, Value>) -> Result<(), String>,
) -> Result<(), String> {
    fs::create_dir_all(runtime_root).map_err(|error| {
        format!(
            "failed to create notslack runtime state directory {}: {error}",
            runtime_root.display()
        )
    })?;
    let lock_path = runtime_root.join(RUNTIME_STATE_LOCK_FILE);
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "failed to open notslack runtime state lock {}: {error}",
                lock_path.display()
            )
        })?;
    lock.lock_exclusive().map_err(|error| {
        format!(
            "failed to lock notslack runtime state {}: {error}",
            lock_path.display()
        )
    })?;

    let path = runtime_state_path(runtime_root);
    let result = (|| {
        let mut state = load_runtime_state_from_path(&path)?;
        let root = state
            .as_object_mut()
            .ok_or_else(|| "notslack runtime state root must be a JSON object".to_string())?;
        mutation(root)?;
        save_runtime_state_to_path(&path, &state)
    })();
    let unlock_result = FileExt::unlock(&lock).map_err(|error| {
        format!(
            "failed to unlock notslack runtime state {}: {error}",
            lock_path.display()
        )
    });
    result.and(unlock_result)
}

fn load_runtime_state_from_path(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read notslack runtime state {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "failed to decode notslack runtime state {}: {error}",
            path.display()
        )
    })
}

fn save_runtime_state_to_path(path: &Path, state: &Value) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "notslack runtime state path has no parent directory: {}",
            path.display()
        )
    })?;
    let contents = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to encode notslack runtime state: {error}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "failed to create temporary notslack runtime state file in {}: {error}",
            parent.display()
        )
    })?;
    temporary.write_all(contents.as_bytes()).map_err(|error| {
        format!(
            "failed to write temporary notslack runtime state file in {}: {error}",
            parent.display()
        )
    })?;
    temporary.as_file_mut().sync_all().map_err(|error| {
        format!(
            "failed to sync temporary notslack runtime state file in {}: {error}",
            parent.display()
        )
    })?;
    temporary.persist(path).map_err(|error| {
        format!(
            "failed to atomically replace notslack runtime state {}: {}",
            path.display(),
            error.error
        )
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync notslack runtime state directory {}: {error}",
                parent.display()
            )
        })
}

fn runtime_state_path(runtime_root: &Path) -> PathBuf {
    runtime_root.join(RUNTIME_STATE_FILE)
}
