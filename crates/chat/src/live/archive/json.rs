use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

#[cfg(test)]
use std::io::Write;

use flate2::read::GzDecoder;
#[cfg(test)]
use flate2::{write::GzEncoder, Compression};
use serde::de::DeserializeOwned;
#[cfg(test)]
use serde::Serialize;

pub(super) fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let resolved_path = resolve_json_path(path)?;
    let bytes = read_existing_json_bytes(&resolved_path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse {}: {error}", resolved_path.display()))
}

#[cfg(test)]
pub(super) fn save_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_json_bytes(path, &body)
}

fn resolve_json_path(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return Ok(path.to_path_buf());
    }

    let gzip_path = path.with_added_extension("gz");
    if gzip_path != path && gzip_path.exists() {
        return Ok(gzip_path);
    }

    let plain_path = with_removed_gz_extension(path);
    if plain_path != path && plain_path.exists() {
        return Ok(plain_path);
    }

    Err(format!(
        "failed to read {}: file does not exist",
        path.display()
    ))
}

fn read_existing_json_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if json_uses_gzip(path) {
        let file = File::open(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let mut decoder = GzDecoder::new(file);
        let mut bytes = Vec::new();
        decoder
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        Ok(bytes)
    } else {
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
    }
}

#[cfg(test)]
fn write_json_bytes(path: &Path, body: &[u8]) -> Result<(), String> {
    if json_uses_gzip(path) {
        let file = File::create(path)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder
            .write_all(body)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        encoder
            .finish()
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        Ok(())
    } else {
        fs::write(path, body)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))
    }
}

fn json_uses_gzip(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("gz")
}

fn with_removed_gz_extension(path: &Path) -> PathBuf {
    match (
        path.extension().and_then(|value| value.to_str()),
        path.file_stem().and_then(|value| value.to_str()),
    ) {
        (Some("gz"), Some(stem)) => path.with_file_name(stem),
        _ => path.to_path_buf(),
    }
}
