mod json;
mod normalize;

use std::path::Path;

use crate::model::SlackWorkspace;
use json::load_json;
#[cfg(test)]
use json::save_json_pretty;
use normalize::normalize_slack_archive;

pub fn load_slack_archive(path: &Path) -> Result<SlackWorkspace, String> {
    let workspace = load_json(path)?;
    Ok(normalize_slack_archive(workspace))
}

#[cfg(test)]
mod tests;
