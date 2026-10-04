use super::{
    alpha, build_slack_remote_image_from_parts, div, img, list, point, px, relative, rgb,
    slack_base_icon_radius, slack_dm_peer_local_time_label, slack_icon, slack_palette, AnyElement,
    AppearanceMode, BoxShadow, Context, Div, FluentBuilder, FontWeight, Image, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement,
    SlackAuxPanelQueryBehavior, SlackAuxPanelRow, SlackAuxPanelSection, SlackAuxPanelState,
    SlackComposerFormatAction, SlackComposerTarget, SlackMainTab, SlackPalette, SlackRailView,
    SlackShellIcon, SlackThreadPanelState, StatefulInteractiveElement, Styled, SurfaceState,
    Window, SLACK_COMPOSER_ATTACHMENTS_HEIGHT, SLACK_COMPOSER_INPUT_HEIGHT,
    SLACK_COMPOSER_PICKER_BOTTOM_OFFSET, SLACK_COMPOSER_PICKER_LEFT_INSET,
    SLACK_COMPOSER_PICKER_POPOVER_WIDTH, SLACK_COMPOSER_TOOLBAR_HEIGHT,
    SLACK_EMOJI_PICKER_POPOVER_HEIGHT, SLACK_MAIN_HEADER_HEIGHT,
    SLACK_MENTION_PICKER_POPOVER_HEIGHT, SLACK_TABS_HEIGHT,
};
use crate::ui::{SlackConversationKind, SlackWorkspace};
use gpui::Role;

mod body;
mod bookmark_folder;
mod composer;
mod composer_popover;
mod conversation_files;
mod emoji_picker;
mod header;
mod lists_threads;
mod members;
mod mention_picker;
mod pins;
mod shared_banner;
mod tabs;
mod thread_composer;

pub(in crate::ui::surface) use members::{
    slack_avatar_presence_badge, slack_members_presence_dot, SlackAvatarPresenceBadgeSize,
    SlackAvatarPresenceBadgeSpec,
};

impl SurfaceState {
    pub(crate) fn render_slack_main(&self, cx: &mut Context<Self>) -> AnyElement {
        let composer_presentation = self
            .slack_active_main_composer_context
            .as_ref()
            .map(|context| context.presentation.clone());
        let palette = slack_palette(self.appearance_mode);
        (div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.main_bg))
            .child(self.render_cached_slack_conversation())
            .when_some(
                (self.slack_main_shows_channel_chrome()
                    && self.slack_active_tab == SlackMainTab::Messages
                    && self.has_current_slack_send_target())
                .then_some(composer_presentation)
                .flatten(),
                |this, presentation| {
                    this.child(self.render_slack_composer_panel(&presentation, cx))
                },
            ))
        .into_any_element()
    }

    pub(super) fn render_slack_conversation_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let workspace = self
            .slack_workspace
            .clone()
            .expect("cached Slack conversation requires a workspace");
        let view = cx.entity();
        let external_members_banner = self.render_slack_external_members_banner(&workspace, cx);
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_main_header(&workspace, cx))
            .when(self.slack_main_shows_channel_chrome(), |this| {
                this.child(self.render_slack_main_tabs(&workspace, cx))
            })
            .child(self.render_slack_main_body(&workspace, view, cx))
            .when_some(external_members_banner, |this, banner| this.child(banner))
            .into_any_element()
    }

    fn slack_main_shows_channel_chrome(&self) -> bool {
        true
    }

    pub(super) fn slack_composer_placeholder(&self, workspace: &SlackWorkspace) -> String {
        let self_direct_message = workspace.channel_kind == SlackConversationKind::DirectMessage
            && workspace
                .self_user_id
                .as_deref()
                .is_some_and(|self_user_id| {
                    workspace
                        .sections
                        .iter()
                        .flat_map(|section| section.items.iter())
                        .find(|item| item.target_id.as_str() == workspace.conversation_id)
                        .and_then(|item| item.user_id.as_deref())
                        == Some(self_user_id)
                });
        if self_direct_message {
            return "Jot something down".to_string();
        }
        workspace
            .composer_placeholder
            .strip_prefix("Message to ")
            .map(|channel| format!("Message {channel}"))
            .unwrap_or_else(|| workspace.composer_placeholder.clone())
    }

    fn render_slack_main_header(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        (div()
            .id("slack-primary-view-actions")
            .role(Role::Toolbar)
            .aria_label("Primary view actions")
            .h(px(SLACK_MAIN_HEADER_HEIGHT))
            .pl(px(18.0))
            .pr(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_main_title(workspace, cx))
            .child(self.render_slack_main_actions(workspace, cx)))
        .into_any_element()
    }
}

fn slack_empty_messages(appearance_mode: AppearanceMode) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .rounded(px(12.0))
        .border_1()
        .border_color(rgb(palette.attachment_border))
        .bg(rgb(palette.attachment_bg))
        .px(px(20.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(palette.main_text))
                .child("No messages yet"),
        )
}

fn slack_error_banner(message: &str) -> Div {
    div()
        .mx(px(12.0))
        .mt(px(12.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(0x6e3c39))
        .bg(rgb(0x341d1d))
        .px(px(12.0))
        .py(px(8.0))
        .text_size(px(12.0))
        .line_height(relative(1.35))
        .text_color(rgb(0xf2d8d6))
        .child(message.to_string())
}
