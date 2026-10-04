use std::sync::OnceLock;

use gpui::ElementId;
use uuid::Uuid;

use super::{
    Arc, ListState, Range, SharedString, SlackMessageActionTarget, SlackMessageRenderContext,
    SlackReaction, SlackReactionSkinToneSupport,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionPickerEmoji {
    pub(crate) name: SharedString,
    pub(crate) search_key: SharedString,
    pub(crate) presentation: SlackReactionPickerEmojiPresentation,
    pub(crate) accessibility_label: SharedString,
    pub(crate) skin_tone_support: SlackReactionSkinToneSupport,
}

impl SlackReactionPickerEmoji {
    pub(crate) fn remote_image_url(&self) -> Option<&SharedString> {
        match &self.presentation {
            SlackReactionPickerEmojiPresentation::Glyph(_) => None,
            SlackReactionPickerEmojiPresentation::RemoteImage { url } => Some(url),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackReactionPickerEmojiPresentation {
    Glyph(SharedString),
    RemoteImage { url: SharedString },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SlackReactionPickerListRow {
    Heading(SharedString),
    EmojiGrid(Range<usize>),
    Empty(SharedString),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionPickerContent {
    pub(crate) emojis: Arc<[SlackReactionPickerEmoji]>,
    pub(crate) rows: Arc<[SlackReactionPickerListRow]>,
}

pub(crate) const SLACK_REACTION_PICKER_CATEGORY_COUNT: usize = 10;
pub(crate) const SLACK_STANDARD_REACTION_PICKER_CATEGORY_COUNT: usize = 8;

pub(crate) struct SlackStandardReactionPickerEntry {
    source: &'static emojis::Emoji,
    emoji: SlackReactionPickerEmoji,
}

impl SlackStandardReactionPickerEntry {
    pub(crate) fn new(source: &'static emojis::Emoji, emoji: SlackReactionPickerEmoji) -> Self {
        Self { source, emoji }
    }

    pub(crate) fn source(&self) -> &'static emojis::Emoji {
        self.source
    }

    pub(crate) fn emoji(&self) -> &SlackReactionPickerEmoji {
        &self.emoji
    }
}

pub(crate) struct SlackStandardReactionPickerCategory {
    entries: Arc<[SlackStandardReactionPickerEntry]>,
    content: SlackReactionPickerContent,
}

impl SlackStandardReactionPickerCategory {
    pub(crate) fn new(
        entries: Arc<[SlackStandardReactionPickerEntry]>,
        content: SlackReactionPickerContent,
    ) -> Self {
        Self { entries, content }
    }

    pub(crate) fn entries(&self) -> &Arc<[SlackStandardReactionPickerEntry]> {
        &self.entries
    }

    pub(crate) fn content(&self) -> &SlackReactionPickerContent {
        &self.content
    }
}

pub(crate) struct SlackStandardReactionPickerCatalog {
    categories: [OnceLock<SlackStandardReactionPickerCategory>;
        SLACK_STANDARD_REACTION_PICKER_CATEGORY_COUNT],
}

impl Default for SlackStandardReactionPickerCatalog {
    fn default() -> Self {
        Self {
            categories: std::array::from_fn(|_| OnceLock::new()),
        }
    }
}

impl SlackStandardReactionPickerCatalog {
    pub(crate) fn category(
        &self,
        category_index: usize,
        initialize: impl FnOnce() -> SlackStandardReactionPickerCategory,
    ) -> &SlackStandardReactionPickerCategory {
        assert!(
            (1..=SLACK_STANDARD_REACTION_PICKER_CATEGORY_COUNT).contains(&category_index),
            "standard Slack reaction picker category index must be between 1 and 8"
        );
        self.categories[category_index - 1].get_or_init(initialize)
    }
}

pub(crate) struct SlackReactionPickerCatalog {
    team_id: Option<SharedString>,
    revision: Option<[u8; 32]>,
    frequent: Arc<[SlackReactionPickerEmoji]>,
    custom: Arc<[SlackReactionPickerEmoji]>,
    frequent_category: OnceLock<SlackReactionPickerContent>,
    custom_category: OnceLock<SlackReactionPickerContent>,
}

pub(crate) struct SlackPreparedReactionPickerCatalog {
    pub(crate) frequent: Arc<[SlackReactionPickerEmoji]>,
    pub(crate) custom: Arc<[SlackReactionPickerEmoji]>,
}

impl Default for SlackReactionPickerCatalog {
    fn default() -> Self {
        Self {
            team_id: None,
            revision: None,
            frequent: Arc::default(),
            custom: Arc::default(),
            frequent_category: OnceLock::new(),
            custom_category: OnceLock::new(),
        }
    }
}

impl SlackReactionPickerCatalog {
    pub(crate) fn sync_team(&mut self, team_id: &str) -> bool {
        if self.team_id.as_deref() == Some(team_id) {
            return false;
        }
        *self = Self {
            team_id: Some(team_id.to_string().into()),
            ..Self::default()
        };
        true
    }

    pub(crate) fn install(
        &mut self,
        team_id: &str,
        revision: [u8; 32],
        prepare: impl FnOnce() -> SlackPreparedReactionPickerCatalog,
    ) -> bool {
        assert_eq!(
            self.team_id.as_deref(),
            Some(team_id),
            "Slack reaction catalog must match the active team"
        );
        if self.revision == Some(revision) {
            return false;
        }
        let SlackPreparedReactionPickerCatalog { frequent, custom } = prepare();
        self.revision = Some(revision);
        self.frequent = frequent;
        self.custom = custom;
        self.frequent_category = OnceLock::new();
        self.custom_category = OnceLock::new();
        true
    }

    pub(crate) fn frequent(&self) -> &Arc<[SlackReactionPickerEmoji]> {
        &self.frequent
    }

    pub(crate) fn custom(&self) -> &Arc<[SlackReactionPickerEmoji]> {
        &self.custom
    }

    pub(crate) fn is_installed(&self) -> bool {
        self.revision.is_some()
    }

    pub(crate) fn team_category_content(
        &self,
        category_index: usize,
        initialize: impl FnOnce() -> SlackReactionPickerContent,
    ) -> &SlackReactionPickerContent {
        match category_index {
            0 => self.frequent_category.get_or_init(initialize),
            9 => self.custom_category.get_or_init(initialize),
            _ => panic!("only team-dependent Slack reaction picker categories may be cached here"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionCatalogLoad {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackReactionPickerAnchor {
    HoverAction,
    ReactionBar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackReactionPickerIdentity {
    target: Arc<SlackMessageActionTarget>,
    render_context: SlackMessageRenderContext,
    anchor: SlackReactionPickerAnchor,
    element_id: ElementId,
}

impl SlackReactionPickerIdentity {
    pub(crate) fn new(
        target: Arc<SlackMessageActionTarget>,
        render_context: SlackMessageRenderContext,
        anchor: SlackReactionPickerAnchor,
    ) -> Self {
        let mut element_id = [0; 20];
        element_id[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        Self {
            target,
            render_context,
            anchor,
            element_id: ElementId::OpaqueId(element_id),
        }
    }

    pub(crate) fn element_id(&self) -> ElementId {
        self.element_id.clone()
    }

    pub(crate) fn matches_surface(&self, other: &Self) -> bool {
        self.target == other.target
            && self.render_context == other.render_context
            && self.anchor == other.anchor
    }

    pub(crate) fn target(&self) -> &Arc<SlackMessageActionTarget> {
        &self.target
    }

    pub(crate) fn render_context(&self) -> SlackMessageRenderContext {
        self.render_context
    }

    pub(crate) fn anchor(&self) -> SlackReactionPickerAnchor {
        self.anchor
    }
}

#[derive(Clone)]
pub(crate) struct SlackReactionPickerSource {
    pub(crate) identity: SlackReactionPickerIdentity,
    pub(crate) reactions: Arc<[SlackReaction]>,
}

impl SlackReactionPickerSource {
    pub(crate) fn new(
        target: Arc<SlackMessageActionTarget>,
        reactions: Arc<[SlackReaction]>,
        render_context: SlackMessageRenderContext,
        anchor: SlackReactionPickerAnchor,
    ) -> Self {
        Self {
            identity: SlackReactionPickerIdentity::new(target, render_context, anchor),
            reactions,
        }
    }
}

pub(crate) struct SlackReactionPickerSelection {
    pub(crate) query: String,
    pub(crate) category_index: usize,
}

#[derive(Clone)]
pub(crate) struct SlackReactionPickerState {
    pub(crate) identity: SlackReactionPickerIdentity,
    pub(crate) reactions: Arc<[SlackReaction]>,
    pub(crate) query: String,
    pub(crate) category_index: usize,
    pub(crate) emojis: Arc<[SlackReactionPickerEmoji]>,
    pub(crate) rows: Arc<[SlackReactionPickerListRow]>,
    pub(crate) list_state: ListState,
}

impl SlackReactionPickerState {
    pub(crate) fn identity(&self) -> SlackReactionPickerIdentity {
        self.identity.clone()
    }

    pub(crate) fn source(&self) -> SlackReactionPickerSource {
        SlackReactionPickerSource {
            identity: self.identity.clone(),
            reactions: self.reactions.clone(),
        }
    }
}
