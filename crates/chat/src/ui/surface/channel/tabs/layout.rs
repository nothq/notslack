use std::sync::Arc;

use crate::ui::surface::{
    PreparedSlackConversationTab, SlackConversationTabAction, SlackConversationTabCapabilities,
    SlackShellIcon, SurfaceState, SLACK_CONVERSATION_TAB_MESSAGES_KEY,
    SLACK_CONVERSATION_TAB_MORE_KEY,
};
use crate::ui::{px, Context, FontWeight, Window};
use crate::ui::{SlackConversationTab, SlackConversationTabTarget};
use gpui::{Bounds, Pixels, SharedString, TextRun, TruncateFrom};

use super::{
    SLACK_CONVERSATION_TABS_ADD_EDIT_RESERVE, SLACK_CONVERSATION_TABS_LEFT_PADDING,
    SLACK_CONVERSATION_TABS_MORE_EXTRA_WIDTH, SLACK_CONVERSATION_TABS_RIGHT_PADDING,
    SLACK_CONVERSATION_TAB_FONT_SIZE, SLACK_CONVERSATION_TAB_GAP,
    SLACK_CONVERSATION_TAB_HORIZONTAL_PADDING, SLACK_CONVERSATION_TAB_ICON_SIZE,
    SLACK_CONVERSATION_TAB_MAX_WIDTH,
};

struct SlackConversationTabPreparation {
    key: SharedString,
    element_id: SharedString,
    label: SharedString,
    icon: SlackShellIcon,
    action: SlackConversationTabAction,
    enabled: bool,
}

impl SurfaceState {
    pub(super) fn update_slack_conversation_tabs_layout(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let workspace = self.slack_workspace.clone();
        let source_tabs = workspace
            .as_ref()
            .map_or(&[][..], |workspace| workspace.tabs.as_slice());
        let capabilities =
            SlackConversationTabCapabilities::from(self.slack_workspace_api_capabilities);
        let mut font = window.text_style().font();
        font.weight = FontWeight::BOLD;
        let scale_factor_bits = window.scale_factor().to_bits();
        let source_changed = !self
            .slack_conversation_tabs_cache
            .source_matches(source_tabs, capabilities);
        let font_changed = self.slack_conversation_tabs_cache.font.as_ref() != Some(&font)
            || self.slack_conversation_tabs_cache.scale_factor_bits != scale_factor_bits;
        let bounds_changed = self.slack_conversation_tabs_cache.bounds != Some(bounds);
        if !source_changed && !font_changed && !bounds_changed {
            return false;
        }
        let width_changed = self
            .slack_conversation_tabs_cache
            .bounds
            .is_none_or(|previous| previous.size.width != bounds.size.width);

        if source_changed || font_changed {
            self.slack_conversation_tabs_cache.tabs =
                Self::prepare_slack_conversation_tabs(source_tabs, capabilities, &font, window);
            self.slack_conversation_tabs_cache.more_width =
                Self::measure_slack_conversation_tab_label("More", &font, window)
                    + SLACK_CONVERSATION_TABS_MORE_EXTRA_WIDTH;
            self.slack_conversation_tabs_cache.font = Some(font);
            self.slack_conversation_tabs_cache.scale_factor_bits = scale_factor_bits;
            if source_changed {
                self.slack_conversation_tabs_cache.source_tabs = source_tabs.to_vec().into();
                self.slack_conversation_tabs_cache.capabilities = capabilities;
                self.slack_conversation_tabs_overflow_open = false;
                self.slack_conversation_tabs_overflow_selected_index = None;
                self.slack_conversation_tabs_overflow_focus_pending = false;
            }
            self.sync_slack_conversation_tab_focus_handles(cx);
        }

        self.slack_conversation_tabs_cache.bounds = Some(bounds);
        if source_changed || font_changed || width_changed {
            self.partition_slack_conversation_tabs(bounds.size.width.as_f32());
            self.normalize_slack_conversation_tab_roving_key();
        }
        true
    }

    fn sync_slack_conversation_tab_focus_handles(&mut self, cx: &mut Context<Self>) {
        let tabs = self.slack_conversation_tabs_cache.tabs.clone();
        self.slack_conversation_tab_focus_handles
            .retain(|key, _| tabs.iter().any(|tab| tab.key == *key));
        for tab in tabs.iter() {
            self.slack_conversation_tab_focus_handles
                .entry(tab.key.clone())
                .or_insert_with(|| cx.focus_handle());
        }
    }

    fn prepare_slack_conversation_tabs(
        source_tabs: &[SlackConversationTab],
        capabilities: SlackConversationTabCapabilities,
        font: &gpui::Font,
        window: &mut Window,
    ) -> Arc<[PreparedSlackConversationTab]> {
        let mut tabs = Vec::with_capacity(source_tabs.len() + 2);
        tabs.push(Self::prepare_slack_conversation_tab(
            SlackConversationTabPreparation {
                key: SLACK_CONVERSATION_TAB_MESSAGES_KEY.into(),
                element_id: "slack-conversation-tab-messages".into(),
                label: "Messages".into(),
                icon: SlackShellIcon::MessageFilled,
                action: SlackConversationTabAction::Messages,
                enabled: true,
            },
            font,
            window,
        ));
        if !slack_conversation_tabs_include_canvas(source_tabs) {
            tabs.push(Self::prepare_slack_conversation_tab(
                SlackConversationTabPreparation {
                    key: "add-canvas".into(),
                    element_id: "slack-conversation-tab-add-canvas".into(),
                    label: "Add canvas".into(),
                    icon: SlackShellIcon::Canvas,
                    action: SlackConversationTabAction::AddCanvas,
                    enabled: false,
                },
                font,
                window,
            ));
        }
        for tab in source_tabs {
            if tab.is_disabled
                || tab.id.is_empty()
                || tab.label.trim().is_empty()
                || !capabilities.supports(&tab.target)
            {
                continue;
            }
            let Some((icon, action)) = slack_conversation_tab_presentation(tab) else {
                continue;
            };
            tabs.push(Self::prepare_slack_conversation_tab(
                SlackConversationTabPreparation {
                    key: format!("source:{}", tab.id).into(),
                    element_id: format!("slack-conversation-tab-{}", tab.id).into(),
                    label: tab.label.clone().into(),
                    icon,
                    action,
                    enabled: true,
                },
                font,
                window,
            ));
        }
        tabs.into()
    }

    fn prepare_slack_conversation_tab(
        preparation: SlackConversationTabPreparation,
        font: &gpui::Font,
        window: &mut Window,
    ) -> PreparedSlackConversationTab {
        let SlackConversationTabPreparation {
            key,
            element_id,
            label,
            icon,
            action,
            enabled,
        } = preparation;
        let width = (Self::measure_slack_conversation_tab_label(label.as_ref(), font, window)
            + SLACK_CONVERSATION_TAB_ICON_SIZE
            + SLACK_CONVERSATION_TAB_GAP
            + SLACK_CONVERSATION_TAB_HORIZONTAL_PADDING * 2.0)
            .min(SLACK_CONVERSATION_TAB_MAX_WIDTH);
        let overflow_element_id = format!("{}-overflow", element_id.as_ref()).into();
        PreparedSlackConversationTab {
            key,
            element_id,
            overflow_element_id,
            label,
            icon,
            action: Arc::new(action),
            enabled,
            width,
        }
    }

    fn measure_slack_conversation_tab_label(
        label: &str,
        font: &gpui::Font,
        window: &mut Window,
    ) -> f32 {
        let font_size = px(SLACK_CONVERSATION_TAB_FONT_SIZE);
        let run = TextRun {
            len: label.len(),
            font: font.clone(),
            color: window.text_style().color,
            ..Default::default()
        };
        let shaped_width = f32::from(
            window
                .text_system()
                .layout_line(label, font_size, &[run], None)
                .width,
        );
        let mut truncation_width = shaped_width;
        let mut line_wrapper = window.text_system().line_wrapper(font.clone(), font_size);
        while line_wrapper
            .should_truncate_line(label, px(truncation_width), "\u{2026}", TruncateFrom::End)
            .is_some()
        {
            truncation_width = truncation_width.floor() + 1.0;
        }
        truncation_width
    }

    fn partition_slack_conversation_tabs(&mut self, container_width: f32) {
        let tabs = &self.slack_conversation_tabs_cache.tabs;
        if tabs.is_empty() {
            self.slack_conversation_tabs_cache.visible_count = 0;
            return;
        }
        let available_width = (container_width
            - SLACK_CONVERSATION_TABS_LEFT_PADDING
            - SLACK_CONVERSATION_TABS_RIGHT_PADDING
            - SLACK_CONVERSATION_TAB_GAP
            - SLACK_CONVERSATION_TABS_ADD_EDIT_RESERVE)
            .max(0.0);
        let all_tabs_width = tabs.iter().map(|tab| tab.width).sum::<f32>()
            + SLACK_CONVERSATION_TAB_GAP * tabs.len().saturating_sub(1) as f32;
        if tabs.len() == 1 || all_tabs_width <= available_width {
            self.slack_conversation_tabs_cache.visible_count = tabs.len();
            self.slack_conversation_tabs_overflow_open = false;
            self.slack_conversation_tabs_overflow_selected_index = None;
            self.slack_conversation_tabs_overflow_focus_pending = false;
            return;
        }

        let visible_width = (available_width
            - self.slack_conversation_tabs_cache.more_width * 2.0
            - SLACK_CONVERSATION_TAB_GAP)
            .max(0.0);
        let mut used_width = tabs[0].width;
        let mut visible_count = 1;
        for tab in tabs.iter().skip(1) {
            let next_width = used_width + SLACK_CONVERSATION_TAB_GAP + tab.width;
            if next_width > visible_width {
                break;
            }
            used_width = next_width;
            visible_count += 1;
        }
        self.slack_conversation_tabs_cache.visible_count = visible_count;
    }

    fn normalize_slack_conversation_tab_roving_key(&mut self) {
        let visible_count = self.slack_conversation_tabs_cache.visible_count;
        let tabs = self.slack_conversation_tabs_cache.tabs.clone();
        let has_overflow = self.slack_conversation_tabs_cache.has_overflow();
        if !has_overflow {
            self.slack_conversation_tabs_overflow_open = false;
            self.slack_conversation_tabs_overflow_selected_index = None;
            self.slack_conversation_tabs_overflow_focus_pending = false;
        }
        let current = self.slack_conversation_tab_roving_key.as_ref();
        let current_visible = tabs
            .iter()
            .take(visible_count)
            .any(|tab| tab.key.as_ref() == current);
        if current_visible || (has_overflow && current == SLACK_CONVERSATION_TAB_MORE_KEY) {
            return;
        }
        let active_key = tabs
            .iter()
            .take(visible_count)
            .find(|tab| self.slack_conversation_tab_is_active(tab))
            .map(|tab| tab.key.clone());
        self.slack_conversation_tab_roving_key =
            active_key.unwrap_or_else(|| SLACK_CONVERSATION_TAB_MESSAGES_KEY.into());
    }
}

fn slack_conversation_tabs_include_canvas(tabs: &[SlackConversationTab]) -> bool {
    tabs.iter().any(|tab| {
        !tab.is_disabled && matches!(&tab.target, SlackConversationTabTarget::Canvas { .. })
    })
}

fn slack_conversation_tab_presentation(
    tab: &SlackConversationTab,
) -> Option<(SlackShellIcon, SlackConversationTabAction)> {
    match &tab.target {
        SlackConversationTabTarget::Canvas {
            permalink: Some(permalink),
            ..
        } if !permalink.trim().is_empty() => Some((
            SlackShellIcon::Canvas,
            SlackConversationTabAction::CanvasLink {
                tab: tab.clone(),
                permalink: permalink.clone().into(),
            },
        )),
        SlackConversationTabTarget::Folder { bookmark_id } if !bookmark_id.is_empty() => Some((
            SlackShellIcon::Folder,
            SlackConversationTabAction::BookmarkFolder(tab.clone()),
        )),
        SlackConversationTabTarget::Files => {
            Some((SlackShellIcon::Files, SlackConversationTabAction::Files))
        }
        SlackConversationTabTarget::Pins => {
            Some((SlackShellIcon::Pins, SlackConversationTabAction::Pins))
        }
        SlackConversationTabTarget::Canvas { .. }
        | SlackConversationTabTarget::Folder { .. }
        | SlackConversationTabTarget::Unsupported { .. } => None,
    }
}
