use super::super::super::super::{
    div, img, px, rgb, slack_base_icon_radius, slack_icon, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackHistoryEntry,
    SlackSearchUnreadDraftPosition, SlackShellIcon, StatefulInteractiveElement, Styled,
    SurfaceState,
};
use super::super::{consume_slack_search_action_key, slack_search_overlay_palette};
use super::{
    SlackSearchOptionPosition, SLACK_SEARCH_RECENT_HEADING_HEIGHT, SLACK_SEARCH_RECENT_ROW_HEIGHT,
};
use crate::ui::SlackConversationKind;
use gpui::{Role, Stateful};

#[derive(Clone, Copy)]
struct SlackSearchRecentPlacePosition {
    history_index: usize,
    option_index: usize,
    option_count: usize,
}

impl SurfaceState {
    pub(super) fn render_slack_search_empty_sections(
        &self,
        unread_drafts: &[SlackSearchUnreadDraftPosition],
        history_indices: &[usize],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        if unread_drafts.is_empty() && history_indices.is_empty() {
            return slack_search_recent_empty(palette.secondary_text);
        }
        let option_count = unread_drafts.len() + history_indices.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(!unread_drafts.is_empty(), |this| {
                this.child(slack_search_section_heading(
                    "Unreads & drafts",
                    palette.secondary_text,
                ))
                .children(unread_drafts.iter().copied().enumerate().map(
                    |(option_index, position)| {
                        self.render_slack_search_unread_draft(
                            position,
                            option_index,
                            option_count,
                            cx,
                        )
                    },
                ))
            })
            .when(!history_indices.is_empty(), |this| {
                this.child(slack_search_section_heading(
                    "Recent places",
                    palette.secondary_text,
                ))
                .children(history_indices.iter().copied().enumerate().map(
                    |(recent_index, history_index)| {
                        let entry = self
                            .slack_conversation_history
                            .get(history_index)
                            .expect("Slack recent-place history index must exist");
                        self.render_slack_search_recent_place(
                            entry,
                            SlackSearchRecentPlacePosition {
                                history_index,
                                option_index: unread_drafts.len() + recent_index,
                                option_count,
                            },
                            cx,
                        )
                    },
                ))
            })
    }

    fn render_slack_search_unread_draft(
        &self,
        position: SlackSearchUnreadDraftPosition,
        option_index: usize,
        option_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let item = self
            .slack_workspace()
            .and_then(|workspace| workspace.sections.get(position.section_index))
            .and_then(|section| section.items.get(position.item_index))
            .expect("Slack unread/draft search position must exist");
        let palette = slack_search_overlay_palette(self.appearance_mode);
        let selected = self.slack_search_selected_option == option_index;
        let conversation_id = item.target_id.clone();
        self.slack_search_unread_draft_base(
            item,
            position,
            SlackSearchOptionPosition {
                option_index,
                option_count,
                selected,
            },
            cx,
        )
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.hover_bg)))
        .focus_visible(move |style| style.bg(rgb(palette.hover_bg)))
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            if *hovered {
                this.select_slack_search_option(option_index, cx);
            }
        }))
        .on_click(cx.listener({
            let conversation_id = conversation_id.clone();
            move |this, _, _, cx| {
                this.select_slack_conversation(&conversation_id, cx);
            }
        }))
        .on_key_down(cx.listener(move |this, event, window, cx| {
            if consume_slack_search_action_key(event, window, cx) {
                this.select_slack_conversation(&conversation_id, cx);
            }
        }))
    }

    fn slack_search_unread_draft_base(
        &self,
        item: &crate::ui::SlackSidebarItem,
        position: SlackSearchUnreadDraftPosition,
        option: SlackSearchOptionPosition,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let SlackSearchOptionPosition {
            option_index,
            option_count,
            selected,
        } = option;
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .id(format!("slack-search-unread-draft-{}", item.target_id))
            .role(Role::ListBoxOption)
            .aria_label(format!(
                "Open {}{}{}",
                item.label,
                if item.unread { ", unread" } else { "" },
                if position.has_draft {
                    ", has draft"
                } else {
                    ""
                },
            ))
            .aria_selected(selected)
            .aria_position_in_set(option_index + 1)
            .aria_size_of_set(option_count)
            .focusable()
            .tab_stop(selected)
            .h(px(SLACK_SEARCH_RECENT_ROW_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .when(selected, |this| this.bg(rgb(palette.selected_bg)))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(22.0))
            .text_color(rgb(palette.text))
            .child(self.render_slack_search_unread_draft_icon(item, palette.text, cx))
            .child(
                slack_search_recent_label(item.label.clone().into())
                    .when(item.unread, |this| this.font_weight(FontWeight::BOLD)),
            )
            .when(position.has_draft, |this| {
                this.child(
                    div()
                        .ml(px(8.0))
                        .flex_none()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.secondary_text))
                        .child("Draft"),
                )
            })
    }

    fn render_slack_search_unread_draft_icon(
        &self,
        item: &crate::ui::SlackSidebarItem,
        text_color: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        let icon = item
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
            .map_or_else(
                || {
                    slack_icon(
                        match item.target_kind {
                            SlackConversationKind::Channel => SlackShellIcon::HashSmall,
                            SlackConversationKind::PrivateChannel => SlackShellIcon::LockSmall,
                            SlackConversationKind::GroupMessage => SlackShellIcon::People,
                            SlackConversationKind::DirectMessage
                            | SlackConversationKind::Unknown => SlackShellIcon::Dm,
                        },
                        text_color,
                        20.0,
                        cx,
                    )
                },
                |image| {
                    img(image)
                        .size(px(20.0))
                        .rounded(slack_base_icon_radius(20.0))
                        .into_any_element()
                },
            );
        div()
            .w(px(36.0))
            .flex_none()
            .flex()
            .items_center()
            .child(icon)
    }

    fn render_slack_search_recent_place(
        &self,
        entry: &SlackHistoryEntry,
        position: SlackSearchRecentPlacePosition,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        let selected = self.slack_search_selected_option == position.option_index;
        div()
            .id(("slack-search-recent-place", position.history_index))
            .role(Role::ListBoxOption)
            .aria_label(entry.accessibility_label.clone())
            .aria_selected(selected)
            .aria_position_in_set(position.option_index + 1)
            .aria_size_of_set(position.option_count)
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_SEARCH_RECENT_ROW_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .when(selected, |this| this.bg(rgb(palette.selected_bg)))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.hover_bg)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.select_slack_search_option(position.option_index, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_history_menu_index(position.history_index, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_history_menu_index(position.history_index, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(16.0))
            .line_height(px(22.0))
            .text_color(rgb(palette.text))
            .child(self.render_slack_search_recent_icon(entry, palette.text, cx))
            .child(slack_search_recent_label(entry.label.clone()))
    }

    fn render_slack_search_recent_icon(
        &self,
        entry: &SlackHistoryEntry,
        text_color: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        let icon = entry
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
            .map_or_else(
                || slack_icon(entry.icon, text_color, 20.0, cx),
                |image| {
                    img(image)
                        .size(px(20.0))
                        .rounded(slack_base_icon_radius(20.0))
                        .into_any_element()
                },
            );
        div()
            .w(px(36.0))
            .flex_none()
            .flex()
            .items_center()
            .child(icon)
    }
}

fn slack_search_recent_empty(text_color: u32) -> Div {
    div()
        .size_full()
        .px(px(16.0))
        .flex()
        .items_center()
        .text_size(px(13.0))
        .text_color(rgb(text_color))
        .child("Type a query to search messages")
}

fn slack_search_section_heading(label: &'static str, text_color: u32) -> Div {
    div()
        .h(px(SLACK_SEARCH_RECENT_HEADING_HEIGHT))
        .flex_none()
        .px(px(16.0))
        .flex()
        .items_center()
        .text_size(px(13.0))
        .text_color(rgb(text_color))
        .child(label)
}

fn slack_search_recent_label(label: gpui::SharedString) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(label)
}
