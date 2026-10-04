use std::thread;

use crate::model::{SlackConversationTab, SlackConversationTabTarget};
use serde::Deserialize;

use super::SlackLiveWorkspaceLoader;

#[derive(Clone, Debug)]
pub(super) struct SlackCanvasTabMetadata {
    title: String,
    permalink: String,
}

impl SlackLiveWorkspaceLoader {
    pub(crate) fn prepare_conversation_tabs(&self, tabs: &mut [SlackConversationTab]) {
        if let Err(error) = self.try_prepare_conversation_tabs(tabs) {
            eprintln!("[notslack-slack-canvas-tab-metadata] prepare_error={error}");
        }
    }

    pub(crate) fn remember_conversation_tab_metadata(&self, tabs: &[SlackConversationTab]) {
        if let Err(error) = self.try_remember_conversation_tab_metadata(tabs) {
            eprintln!("[notslack-slack-canvas-tab-metadata] cache_seed_error={error}");
        }
    }

    fn try_prepare_conversation_tabs(
        &self,
        tabs: &mut [SlackConversationTab],
    ) -> Result<(), String> {
        for tab in tabs.iter_mut() {
            normalize_conversation_tab_label(tab);
        }
        self.try_remember_conversation_tab_metadata(tabs)?;
        let metadata = self
            .canvas_tab_metadata_cache
            .lock()
            .map_err(|_| "Slack canvas tab metadata cache mutex poisoned".to_string())?;
        let canvas_file_ids = tabs
            .iter_mut()
            .filter_map(|tab| {
                let SlackConversationTabTarget::Canvas {
                    file_id,
                    title,
                    permalink,
                    ..
                } = &mut tab.target
                else {
                    return None;
                };
                if let Some(resolved) = metadata.get(file_id) {
                    *title = Some(resolved.title.clone());
                    *permalink = Some(resolved.permalink.clone());
                    if tab.label.trim().is_empty() {
                        tab.label = resolved.title.clone();
                    }
                }
                Some(file_id.clone())
            })
            .collect::<Vec<_>>();
        drop(metadata);
        self.hydrate_canvas_tab_metadata_in_background(canvas_file_ids)
    }

    fn try_remember_conversation_tab_metadata(
        &self,
        tabs: &[SlackConversationTab],
    ) -> Result<(), String> {
        let resolved = tabs.iter().filter_map(|tab| {
            let SlackConversationTabTarget::Canvas {
                file_id,
                title: Some(title),
                permalink: Some(permalink),
                ..
            } = &tab.target
            else {
                return None;
            };
            Some((
                file_id.clone(),
                SlackCanvasTabMetadata {
                    title: title.clone(),
                    permalink: permalink.clone(),
                },
            ))
        });
        self.canvas_tab_metadata_cache
            .lock()
            .map_err(|_| "Slack canvas tab metadata cache mutex poisoned".to_string())?
            .extend(resolved);
        Ok(())
    }

    fn hydrate_canvas_tab_metadata_in_background(
        &self,
        canvas_file_ids: Vec<String>,
    ) -> Result<(), String> {
        let file_ids = {
            let mut started = self
                .canvas_tab_hydration_started
                .lock()
                .map_err(|_| "Slack canvas tab hydration mutex poisoned".to_string())?;
            canvas_file_ids
                .into_iter()
                .filter(|file_id| started.insert(file_id.clone()))
                .collect::<Vec<_>>()
        };
        if file_ids.is_empty() {
            return Ok(());
        }
        let api = self.api.clone();
        let metadata_cache = self.canvas_tab_metadata_cache.clone();
        thread::spawn(move || {
            for file_id in file_ids {
                match load_canvas_tab_metadata(&api, &file_id) {
                    Ok(metadata) => match metadata_cache.lock() {
                        Ok(mut metadata_cache) => {
                            metadata_cache.insert(file_id, metadata);
                        }
                        Err(_) => {
                            eprintln!(
                                "[notslack-slack-canvas-tab-metadata] cache_error=Slack canvas tab metadata cache mutex poisoned"
                            );
                            return;
                        }
                    },
                    Err(error) => {
                        eprintln!(
                            "[notslack-slack-canvas-tab-metadata] file={} hydrate_error={error}",
                            file_id
                        );
                    }
                }
            }
        });
        Ok(())
    }
}

pub(super) fn normalize_conversation_tab_label(tab: &mut SlackConversationTab) {
    if !tab.label.trim().is_empty() {
        return;
    }
    tab.label = match &tab.target {
        SlackConversationTabTarget::Files => "Files & links",
        SlackConversationTabTarget::Pins => "Pins",
        SlackConversationTabTarget::Canvas { .. }
        | SlackConversationTabTarget::Folder { .. }
        | SlackConversationTabTarget::Unsupported { .. } => return,
    }
    .to_string();
}

fn load_canvas_tab_metadata(
    api: &crate::live::api::SlackApiClient,
    file_id: &str,
) -> Result<SlackCanvasTabMetadata, String> {
    let payload = api.post("files.info", &[("file", file_id.to_string())])?;
    let response = serde_json::from_value::<SlackFilesInfoResponse>(payload)
        .map_err(|error| format!("failed to decode Slack files.info response: {error}"))?;
    if response.file.id != file_id {
        return Err(format!(
            "Slack files.info returned file {} for requested {file_id}",
            response.file.id
        ));
    }
    let title = response
        .file
        .title
        .and_then(nonempty)
        .or_else(|| response.file.name.and_then(nonempty))
        .ok_or_else(|| format!("Slack files.info returned file {file_id} without a title"))?;
    let permalink = nonempty(response.file.permalink)
        .ok_or_else(|| format!("Slack files.info returned file {file_id} without a permalink"))?;
    Ok(SlackCanvasTabMetadata { title, permalink })
}

#[derive(Deserialize)]
struct SlackFilesInfoResponse {
    file: SlackCanvasFile,
}

#[derive(Deserialize)]
struct SlackCanvasFile {
    id: String,
    title: Option<String>,
    name: Option<String>,
    permalink: String,
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
