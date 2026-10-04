use super::visibility_item::{
    slack_sidebar_item_leading, slack_sidebar_item_text_color, SlackSidebarItemLeadingSpec,
};
use std::collections::HashMap;
use std::sync::Arc;

use super::{
    div, px, rgb, slack_palette, AppearanceMode, Context, Div, FluentBuilder, FontWeight, Image,
    ParentElement, Styled, SurfaceState, SLACK_SIDEBAR_ROW_HEIGHT,
};
use crate::ui::SlackConversationKind;
use crate::ui::SlackSidebarItem;
use gpui::SharedString;

pub(crate) fn slack_sidebar_item_unread(item: &SlackSidebarItem) -> bool {
    item.unread || item.active
}

pub(crate) fn slack_sidebar_item_badge_count(item: &SlackSidebarItem) -> Option<u32> {
    item.count.filter(|_| !item.active)
}

pub(crate) fn slack_sidebar_item_row(
    item: &SlackSidebarItem,
    appearance_mode: AppearanceMode,
) -> Div {
    slack_sidebar_item_row_with_active(item, appearance_mode, item.active)
}

pub(crate) fn slack_sidebar_item_row_with_active(
    item: &SlackSidebarItem,
    appearance_mode: AppearanceMode,
    active: bool,
) -> Div {
    let palette = slack_palette(appearance_mode);
    let trailing_padding = if slack_sidebar_item_badge_count(item).is_some() {
        6.0
    } else {
        8.0
    };
    div()
        .w_full()
        .h(px(SLACK_SIDEBAR_ROW_HEIGHT))
        .pl(px(24.0))
        .pr(px(trailing_padding))
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_between()
        .when(active, |this| this.bg(rgb(palette.sidebar_active_bg)))
}

pub(crate) struct SlackSidebarItemBodySpec<'a> {
    pub(crate) item: &'a SlackSidebarItem,
    pub(crate) active: bool,
    pub(crate) label: SharedString,
    pub(crate) secondary_context: Option<SharedString>,
    pub(crate) group_count_label: Option<SharedString>,
    pub(crate) unread: bool,
    pub(crate) peer_notifications_paused: bool,
    pub(crate) appearance_mode: AppearanceMode,
    pub(crate) remote_images: &'a HashMap<String, Arc<Image>>,
}

pub(crate) fn slack_sidebar_item_body(
    spec: SlackSidebarItemBodySpec<'_>,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let has_avatar = matches!(
        spec.item.target_kind,
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
    );
    let leading = slack_sidebar_item_leading(
        SlackSidebarItemLeadingSpec {
            item: spec.item,
            active: spec.active,
            label: spec.label.as_ref(),
            group_count_label: spec.group_count_label.clone(),
            has_avatar,
            peer_notifications_paused: spec.peer_notifications_paused,
            appearance_mode: spec.appearance_mode,
            remote_images: spec.remote_images,
        },
        cx,
    );
    div()
        .flex()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .items_center()
        .gap(px(8.0))
        .child(leading)
        .child(slack_sidebar_item_text(spec))
}

fn slack_sidebar_item_text(spec: SlackSidebarItemBodySpec<'_>) -> Div {
    let palette = slack_palette(spec.appearance_mode);
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .min_w(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .min_w(px(0.0))
                .flex_shrink(1.0)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(15.0))
                .line_height(px(SLACK_SIDEBAR_ROW_HEIGHT))
                .font_weight(if spec.active || !spec.unread {
                    FontWeight::NORMAL
                } else {
                    FontWeight::BLACK
                })
                .text_color(rgb(slack_sidebar_item_text_color(
                    spec.item,
                    spec.active,
                    spec.unread,
                    spec.appearance_mode,
                )))
                .child(spec.label),
        )
        .when_some(spec.secondary_context, |this, context| {
            this.child(
                div()
                    .min_w(px(0.0))
                    .flex_shrink(1.0)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(15.0))
                    .line_height(px(SLACK_SIDEBAR_ROW_HEIGHT))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(rgb(palette.sidebar_muted_text))
                    .child(context),
            )
        })
}
