//! The Add emoji dialog: uploading a custom emoji, and adding or removing
//! Slack's emoji packs.

use std::{path::PathBuf, rc::Rc, sync::Arc};

use gpui::{AppContext as _, Image, ImageFormat, PathPromptOptions, SharedString};
use gpui_components::text_input::{TextInput, TextInputProps};

use crate::ui::surface::{
    spawn_background_task_for_entity, Context, SlackAddEmojiDialog, SlackAddEmojiImage,
    SlackAddEmojiTab, SlackEmojiPacksLoad, SurfaceState,
};
use crate::ui::{SlackCustomEmojiUpload, SlackEmojiPack};

/// Slack resizes large images itself, but refuses big uploads outright.
const SLACK_ADD_EMOJI_MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// How many of a pack's emoji its card previews.
pub(crate) const SLACK_EMOJI_PACK_CARD_EMOJI: usize = 8;

impl SurfaceState {
    pub(crate) fn open_slack_add_emoji_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.manage_custom_emoji {
            return;
        }
        self.slack_add_emoji_generation = self.slack_add_emoji_generation.wrapping_add(1);
        let name_input = cx
            .new(|cx| TextInput::new(TextInputProps::single_line("").placeholder(":avocado:"), cx));
        self.slack_add_emoji_dialog = Some(SlackAddEmojiDialog {
            generation: self.slack_add_emoji_generation,
            tab: SlackAddEmojiTab::Custom,
            name_input,
            name: String::new(),
            image: None,
            saving: false,
            error: None,
            packs: SlackEmojiPacksLoad::Loading,
            open_pack: None,
            pack_pending: false,
        });
        self.load_slack_emoji_packs(cx);
        cx.notify();
    }

    pub(crate) fn close_slack_add_emoji_dialog(&mut self, cx: &mut Context<Self>) {
        if self.slack_add_emoji_dialog.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn select_slack_add_emoji_tab(
        &mut self,
        tab: SlackAddEmojiTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(dialog) = self.slack_add_emoji_dialog.as_mut() {
            dialog.tab = tab;
            dialog.open_pack = None;
            dialog.error = None;
            cx.notify();
        }
    }

    /// Wires the name field to the dialog; the name is kept without the
    /// colons people often type around it.
    pub(in crate::ui::surface) fn slack_add_emoji_name_input(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<TextInput>> {
        let dialog = self.slack_add_emoji_dialog.as_ref()?;
        let surface = cx.entity().downgrade();
        let submit_surface = surface.clone();
        let palette = crate::ui::surface::slack_palette(self.appearance_mode);
        let props = TextInputProps::single_line(dialog.name.clone())
            .placeholder(":avocado:")
            .style(gpui_components::text_input::TextInputStyle {
                height: gpui::px(40.0),
                min_height: gpui::px(40.0),
                padding_x: gpui::px(12.0),
                padding_y: gpui::px(10.0),
                radius: gpui::px(8.0),
                background: gpui::rgb(palette.main_bg).into(),
                border: gpui::rgb(palette.reaction_picker_border).into(),
                focused_border: gpui::rgb(palette.reaction_picker_focus_border).into(),
                text: gpui::rgb(palette.main_text).into(),
                placeholder: gpui::rgb(palette.main_secondary_text).into(),
                selection: crate::ui::alpha(0x1264a3, 0.28),
                caret: gpui::rgb(palette.main_text).into(),
                font_size: gpui::px(15.0),
                line_height: gpui::px(20.0),
                font_family: Some("Lato".into()),
            })
            .on_change(Rc::new(move |name, _window, cx| {
                surface
                    .update(cx, |this, cx| {
                        if let Some(dialog) = this.slack_add_emoji_dialog.as_mut() {
                            dialog.name = name;
                            dialog.error = None;
                            cx.notify();
                        }
                    })
                    .ok();
            }))
            .on_submit(Rc::new(move |_window, cx| {
                submit_surface
                    .update(cx, |this, cx| this.save_slack_custom_emoji(cx))
                    .ok();
            }));
        dialog
            .name_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        Some(dialog.name_input.clone())
    }

    pub(crate) fn choose_slack_add_emoji_image(&mut self, cx: &mut Context<Self>) {
        let Some(generation) = self
            .slack_add_emoji_dialog
            .as_ref()
            .map(|dialog| dialog.generation)
        else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Upload Image".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let loaded = cx
                .background_spawn(async move { load_slack_add_emoji_image(path) })
                .await;
            this.update(cx, |this, cx| {
                let Some(dialog) = this
                    .slack_add_emoji_dialog
                    .as_mut()
                    .filter(|dialog| dialog.generation == generation)
                else {
                    return;
                };
                match loaded {
                    Ok(image) => {
                        if dialog.name.is_empty() {
                            dialog.name = slack_emoji_name_from_file_name(&image.file_name);
                        }
                        dialog.image = Some(image);
                        dialog.error = None;
                    }
                    Err(error) => dialog.error = Some(error.into()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn save_slack_custom_emoji(&mut self, cx: &mut Context<Self>) {
        let workspace_api = self.active_slack_workspace_api();
        let Some(dialog) = self.slack_add_emoji_dialog.as_mut() else {
            return;
        };
        if dialog.saving {
            return;
        }
        let name = slack_custom_emoji_name(&dialog.name);
        let upload = match (&dialog.image, name) {
            (None, _) => Err("Choose an image to upload."),
            (Some(_), Err(error)) => Err(error),
            (Some(image), Ok(name)) => Ok(SlackCustomEmojiUpload {
                name,
                file_name: image.file_name.clone(),
                mimetype: image.mimetype.to_string(),
                image: image.bytes.as_ref().clone(),
            }),
        };
        let upload = match upload {
            Ok(upload) => upload,
            Err(error) => {
                dialog.error = Some(error.into());
                cx.notify();
                return;
            }
        };
        let Some(workspace_api) = workspace_api else {
            dialog.error = Some("Connect to Slack to add emoji.".into());
            cx.notify();
            return;
        };
        dialog.saving = true;
        dialog.error = None;
        let generation = dialog.generation;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, upload),
            cx,
            |(workspace_api, upload)| workspace_api.add_slack_custom_emoji(upload),
            move |this, result, cx| {
                let Some(dialog) = this
                    .slack_add_emoji_dialog
                    .as_mut()
                    .filter(|dialog| dialog.generation == generation)
                else {
                    return;
                };
                dialog.saving = false;
                match result {
                    Ok(()) => {
                        this.slack_add_emoji_dialog = None;
                        this.refresh_slack_reaction_catalog(cx);
                    }
                    Err(error) => dialog.error = Some(slack_add_emoji_error(&error).into()),
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn open_slack_emoji_pack(&mut self, pack_id: String, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_add_emoji_dialog.as_mut() else {
            return;
        };
        dialog.open_pack = Some(pack_id);
        dialog.error = None;
        let urls = dialog
            .open_pack()
            .map(|pack| pack.emoji.iter().map(|emoji| emoji.url.clone()).collect())
            .unwrap_or_else(Vec::new);
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        cx.notify();
    }

    pub(crate) fn close_slack_emoji_pack(&mut self, cx: &mut Context<Self>) {
        if let Some(dialog) = self.slack_add_emoji_dialog.as_mut() {
            dialog.open_pack = None;
            dialog.error = None;
            cx.notify();
        }
    }

    /// Adds the open pack to the workspace, or removes it.
    pub(crate) fn toggle_slack_open_emoji_pack(&mut self, cx: &mut Context<Self>) {
        let workspace_api = self.active_slack_workspace_api();
        let Some(dialog) = self.slack_add_emoji_dialog.as_mut() else {
            return;
        };
        if dialog.pack_pending {
            return;
        }
        let Some((pack_id, install)) = dialog
            .open_pack()
            .map(|pack| (pack.id.clone(), !pack.installed))
        else {
            return;
        };
        let Some(workspace_api) = workspace_api else {
            dialog.error = Some("Connect to Slack to add emoji packs.".into());
            cx.notify();
            return;
        };
        dialog.pack_pending = true;
        dialog.error = None;
        let generation = dialog.generation;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, pack_id.clone()),
            cx,
            move |(workspace_api, pack_id)| {
                workspace_api.set_slack_emoji_pack_installed(&pack_id, install)
            },
            move |this, result, cx| {
                let Some(dialog) = this
                    .slack_add_emoji_dialog
                    .as_mut()
                    .filter(|dialog| dialog.generation == generation)
                else {
                    return;
                };
                dialog.pack_pending = false;
                match result {
                    Ok(()) => {
                        if let SlackEmojiPacksLoad::Loaded(packs) = &dialog.packs {
                            dialog.packs = SlackEmojiPacksLoad::Loaded(
                                packs
                                    .iter()
                                    .map(|pack| SlackEmojiPack {
                                        installed: if pack.id == pack_id {
                                            install
                                        } else {
                                            pack.installed
                                        },
                                        ..pack.clone()
                                    })
                                    .collect(),
                            );
                        }
                        dialog.open_pack = None;
                        this.refresh_slack_reaction_catalog(cx);
                    }
                    Err(error) => dialog.error = Some(slack_add_emoji_error(&error).into()),
                }
                cx.notify();
            },
        );
    }

    fn load_slack_emoji_packs(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_add_emoji_dialog.as_ref() else {
            return;
        };
        let generation = dialog.generation;
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            if let Some(dialog) = self.slack_add_emoji_dialog.as_mut() {
                dialog.packs = SlackEmojiPacksLoad::Failed("Connect to Slack to see packs.".into());
            }
            return;
        };
        spawn_background_task_for_entity(
            workspace_api,
            cx,
            |workspace_api| workspace_api.load_slack_emoji_packs(),
            move |this, result, cx| {
                let Some(dialog) = this
                    .slack_add_emoji_dialog
                    .as_mut()
                    .filter(|dialog| dialog.generation == generation)
                else {
                    return;
                };
                let card_urls = result
                    .as_ref()
                    .map(|packs| {
                        packs
                            .iter()
                            .flat_map(|pack| pack.emoji.iter().take(SLACK_EMOJI_PACK_CARD_EMOJI))
                            .map(|emoji| emoji.url.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                dialog.packs = match result {
                    Ok(packs) => SlackEmojiPacksLoad::Loaded(Arc::from(packs)),
                    Err(error) => SlackEmojiPacksLoad::Failed(SharedString::from(error)),
                };
                for url in card_urls {
                    this.enqueue_slack_remote_image_url(url, cx);
                }
                cx.notify();
            },
        );
    }

    /// Reloads the picker's custom emoji after the workspace's set changed.
    fn refresh_slack_reaction_catalog(&mut self, cx: &mut Context<Self>) {
        self.slack_reaction_catalog_request = None;
        self.begin_slack_reaction_catalog_load(cx);
    }
}

fn load_slack_add_emoji_image(path: PathBuf) -> Result<SlackAddEmojiImage, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("emoji")
        .to_string();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let (mimetype, format) = match extension.as_deref() {
        Some("png") => ("image/png", ImageFormat::Png),
        Some("jpg" | "jpeg") => ("image/jpeg", ImageFormat::Jpeg),
        Some("gif") => ("image/gif", ImageFormat::Gif),
        Some("webp") => ("image/webp", ImageFormat::Webp),
        _ => return Err("Please choose a GIF, JPEG, PNG, or WEBP image.".to_string()),
    };
    let size = std::fs::metadata(&path)
        .map_err(|error| format!("Couldn’t read {file_name}: {error}"))?
        .len();
    if size > SLACK_ADD_EMOJI_MAX_IMAGE_BYTES {
        return Err("This image is too big. A smaller file might help?".to_string());
    }
    let bytes =
        std::fs::read(&path).map_err(|error| format!("Couldn’t read {file_name}: {error}"))?;
    let preview = Arc::new(Image::from_bytes(format, bytes.clone()));
    Ok(SlackAddEmojiImage {
        file_name,
        mimetype,
        bytes: Arc::new(bytes),
        preview,
    })
}

/// The name Slack will accept, from what was typed: colons dropped, and only
/// lowercase letters, digits, `-`, `_`, `'` and `.`.
fn slack_custom_emoji_name(typed: &str) -> Result<String, &'static str> {
    let name = typed.trim().trim_matches(':');
    if name.is_empty() {
        return Err("Give your emoji a name.");
    }
    if name.len() > 100 {
        return Err("Names can be at most 100 characters.");
    }
    if !name.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_' | '\'' | '.')
    }) {
        return Err(
            "Names can only have lowercase letters, numbers, dashes, underscores, apostrophes and periods.",
        );
    }
    Ok(name.to_string())
}

/// Suggests a name from the image's file name, as Slack does.
fn slack_emoji_name_from_file_name(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    let mut name = String::new();
    for character in stem.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' {
            name.push(character);
        } else if !name.ends_with('-') && !name.is_empty() {
            name.push('-');
        }
    }
    name.trim_end_matches('-').to_string()
}

/// Slack's error codes for adding emoji, as its own dialog words them.
fn slack_add_emoji_error(error: &str) -> String {
    let code = error.rsplit(": ").next().unwrap_or(error);
    match code {
        "error_name_taken" => "That name is already in use by another emoji.".to_string(),
        "error_name_taken_i18n" => {
            "The international emoji set already has an emoji with that name.".to_string()
        }
        "error_bad_name" | "error_bad_name_i18n" | "error_lower_case_names_only" => {
            "Names must be lowercase, and can’t have spaces, periods or most punctuation."
                .to_string()
        }
        "error_bad_format" => {
            "Please make sure your file is a GIF, JPEG, PNG, or WEBP.".to_string()
        }
        "error_too_big" | "resized_but_still_too_large" | "too_many_frames" => {
            "This image is a bit too big, even after resizing. A smaller file might help?"
                .to_string()
        }
        "error_bad_upload" => {
            "Something prevented your file from being uploaded. Try again?".to_string()
        }
        "no_permission" => "You don’t have permission to add emoji to this workspace.".to_string(),
        "emoji_limit_reached" => "This workspace has reached its emoji limit.".to_string(),
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{slack_add_emoji_error, slack_custom_emoji_name, slack_emoji_name_from_file_name};

    #[test]
    fn typed_names_lose_their_colons_and_must_be_lowercase() {
        assert_eq!(
            slack_custom_emoji_name(" :party-parrot: "),
            Ok("party-parrot".to_string())
        );
        assert!(slack_custom_emoji_name("::").is_err());
        assert!(slack_custom_emoji_name("Party Parrot").is_err());
    }

    #[test]
    fn file_names_suggest_an_emoji_name() {
        assert_eq!(
            slack_emoji_name_from_file_name("Party Parrot (1).gif"),
            "party-parrot-1"
        );
        assert_eq!(slack_emoji_name_from_file_name("cat_wave.png"), "cat_wave");
    }

    #[test]
    fn slack_error_codes_read_as_sentences() {
        assert_eq!(
            slack_add_emoji_error("Slack API emoji.add failed: error_name_taken"),
            "That name is already in use by another emoji."
        );
        assert_eq!(slack_add_emoji_error("network down"), "network down");
    }
}
