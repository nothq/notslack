use std::sync::Arc;

use gpui::{Entity, Image, SharedString};
use gpui_components::text_input::TextInput;

use crate::ui::SlackEmojiPack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackAddEmojiTab {
    Custom,
    Packs,
}

/// The image picked for a new custom emoji.
#[derive(Clone)]
pub(crate) struct SlackAddEmojiImage {
    pub(crate) file_name: String,
    pub(crate) mimetype: &'static str,
    pub(crate) bytes: Arc<Vec<u8>>,
    pub(crate) preview: Arc<Image>,
}

#[derive(Clone)]
pub(crate) enum SlackEmojiPacksLoad {
    Loading,
    Loaded(Arc<[SlackEmojiPack]>),
    Failed(SharedString),
}

/// The Add emoji dialog: uploading a custom emoji, or adding and removing
/// emoji packs.
pub(crate) struct SlackAddEmojiDialog {
    pub(crate) generation: u64,
    pub(crate) tab: SlackAddEmojiTab,
    pub(crate) name_input: Entity<TextInput>,
    pub(crate) name: String,
    pub(crate) image: Option<SlackAddEmojiImage>,
    pub(crate) saving: bool,
    pub(crate) error: Option<SharedString>,
    pub(crate) packs: SlackEmojiPacksLoad,
    /// The pack whose emoji are shown, by id.
    pub(crate) open_pack: Option<String>,
    pub(crate) pack_pending: bool,
}

impl SlackAddEmojiDialog {
    pub(crate) fn open_pack(&self) -> Option<&SlackEmojiPack> {
        let SlackEmojiPacksLoad::Loaded(packs) = &self.packs else {
            return None;
        };
        let id = self.open_pack.as_deref()?;
        packs.iter().find(|pack| pack.id == id)
    }
}
