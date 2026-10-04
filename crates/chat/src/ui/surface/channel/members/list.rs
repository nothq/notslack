use super::{
    div, img, px, rgb, slack_base_icon_radius, slack_members_action_key,
    slack_members_presence_dot, slack_palette, AnyElement, Context, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ListSizingBehavior, ParentElement, StatefulInteractiveElement,
    Styled, SurfaceState, SLACK_MEMBERS_ROW_HEIGHT,
};
use crate::ui::surface::SlackMemberRow;
use gpui::{uniform_list, Div, Role, Stateful};

impl SurfaceState {
    pub(super) fn render_slack_members_dialog_content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.slack_members_rows.is_empty() && self.slack_members_loading {
            return self.render_slack_members_skeleton().into_any_element();
        }
        if self.slack_members_visible_row_indices.is_empty() {
            return self.render_slack_members_empty_state(cx);
        }
        self.render_slack_members_list(cx)
    }

    fn render_slack_members_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let message = if let Some(error) = self.slack_members_error.as_deref() {
            error
        } else if self.slack_members_query.trim().is_empty() {
            "No members found"
        } else {
            "No members match your search"
        };
        let has_error = self.slack_members_error.is_some();
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .px(px(28.0))
            .py(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(15.0))
            .text_color(rgb(slack_palette(self.appearance_mode).main_secondary_text))
            .child(message.to_string())
            .when(has_error, |this| {
                this.child(
                    div()
                        .id("slack-channel-members-retry")
                        .role(Role::Button)
                        .aria_label("Retry loading channel members")
                        .focusable()
                        .tab_stop(true)
                        .h(px(32.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x1264a3))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .child("Retry")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.retry_slack_members(cx);
                        }))
                        .on_key_down(cx.listener(|this, event, window, cx| {
                            if slack_members_action_key(event) {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.retry_slack_members(cx);
                            }
                        })),
                )
            })
            .into_any_element()
    }

    fn render_slack_members_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.slack_members_rows.clone();
        let visible_indices = self.slack_members_visible_row_indices.clone();
        let row_count = visible_indices.len();
        let scroll_handle = self.slack_members_scroll_handle.clone();
        let view = cx.entity();
        div()
            .id("slack-channel-members-listbox")
            .role(Role::ListBox)
            .aria_label("Channel members")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                uniform_list(
                    "slack-channel-members-rows",
                    row_count,
                    move |range, _window, cx| {
                        let visible_start = range.start;
                        let visible_end = range.end;
                        let rows = rows.clone();
                        let visible_indices = visible_indices.clone();
                        view.update(cx, move |this, cx| {
                            this.handle_slack_members_visible_range(visible_start, visible_end, cx);
                            range
                                .map(|visible_index| {
                                    let row_index = *visible_indices
                                        .get(visible_index)
                                        .expect("Slack member visible row index must exist");
                                    let row = rows
                                        .get(row_index)
                                        .expect("prepared Slack member row must exist");
                                    this.render_slack_member_row(row, visible_index, row_count, cx)
                                })
                                .collect::<Vec<_>>()
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .track_scroll(&scroll_handle)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_member_row(
        &self,
        row: &SlackMemberRow,
        visible_index: usize,
        row_count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let profile_enabled = self.slack_workspace_api_capabilities.load_profile;
        let row_element =
            self.render_slack_member_row_element(row, visible_index, row_count, profile_enabled);
        if !profile_enabled {
            return row_element.into_any_element();
        }
        row_element
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_member_row(visible_index, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_members_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_member_row(visible_index, cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_member_row_element(
        &self,
        row: &SlackMemberRow,
        visible_index: usize,
        row_count: usize,
        profile_enabled: bool,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_position_in_set(visible_index + 1)
            .aria_size_of_set(row_count)
            .focusable()
            .tab_stop(profile_enabled)
            .w_full()
            .h(px(SLACK_MEMBERS_ROW_HEIGHT))
            .flex_none()
            .px(px(28.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .when(profile_enabled, |this| {
                this.cursor_pointer().hover(|style| {
                    style.bg(rgb(
                        if self.appearance_mode == crate::ui::AppearanceMode::Dark {
                            0x24262a
                        } else {
                            0xf8f8f8
                        },
                    ))
                })
            })
            .child(self.render_slack_member_avatar(row))
            .child(self.render_slack_member_copy(row, &palette))
    }

    fn render_slack_member_avatar(&self, row: &SlackMemberRow) -> Div {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .size(px(36.0))
                .rounded(slack_base_icon_radius(36.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(36.0)));
        }
        div()
            .size(px(36.0))
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_initials.clone())
    }

    fn render_slack_member_copy(
        &self,
        row: &SlackMemberRow,
        palette: &super::super::SlackPalette,
    ) -> Div {
        let row_background = palette.main_bg;
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .justify_center()
            .child(
                div()
                    .min_w(px(0.0))
                    .h(px(22.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .flex()
                    .items_center()
                    .gap(px(7.0))
                    .text_size(px(15.0))
                    .line_height(px(20.0))
                    .text_color(rgb(palette.main_text))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .text_ellipsis()
                            .font_weight(FontWeight::BOLD)
                            .child(row.primary_label.clone()),
                    )
                    .when_some(row.presence, |this, presence| {
                        this.child(slack_members_presence_dot(presence, row_background))
                    })
                    .when_some(row.display_name.clone(), |this, display_name| {
                        this.child(
                            div()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(display_name),
                        )
                    }),
            )
            .when_some(row.title.clone(), |this, title| {
                this.child(
                    div()
                        .min_w(px(0.0))
                        .h(px(20.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(15.0))
                        .line_height(px(20.0))
                        .text_color(rgb(palette.main_secondary_text))
                        .child(title),
                )
            })
    }
}
