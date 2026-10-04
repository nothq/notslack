use super::super::super::{
    div, img, list, px, rgb, slack_base_icon_radius, slack_palette, AnyElement, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    SlackMessageActionTarget, SlackMessageRenderContext, SlackReactionBarInput, SlackSearchRow,
    SlackSurfaceActionButton, StatefulInteractiveElement, Styled, SurfaceState,
};
use super::{consume_slack_search_action_key, slack_search_overlay_palette};
use crate::ui::SlackMessageSearchSort;
use gpui::Role;
use std::sync::Arc as StdArc;

mod attachments;
mod body;
mod thread;

use body::slack_search_result_metadata;

const SLACK_SEARCH_RESULTS_HORIZONTAL_INSET: f32 = 112.0;
const SLACK_SEARCH_RESULTS_HEADER_HEIGHT: f32 = 123.0;

impl SurfaceState {
    pub(in super::super) fn render_slack_search_results_surface(
        &self,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(rgb(palette.main_bg))
            .flex()
            .flex_col()
            .child(self.render_slack_search_results_header(cx))
            .child(self.render_slack_search_results_content(cx))
    }

    fn render_slack_search_results_header(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_SEARCH_RESULTS_HEADER_HEIGHT))
            .flex_none()
            .px(px(SLACK_SEARCH_RESULTS_HORIZONTAL_INSET))
            .pt(px(18.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .text_size(px(20.0))
                    .text_color(rgb(palette.main_text))
                    .child("Results for: ")
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.message_author))
                            .child(self.slack_search_committed_query.clone()),
                    ),
            )
            .child(self.render_slack_search_refinements(cx))
            .child(
                div()
                    .h(px(25.0))
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child(format!("{} results", self.slack_search_total)),
            )
    }

    fn render_slack_search_refinements(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(50.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .children(
                self.slack_search_refinement_buttons()
                    .map(|(button, selected)| {
                        self.render_slack_search_refinement(button, selected, cx)
                    }),
            )
    }

    fn slack_search_refinement_buttons(&self) -> [(SlackSurfaceActionButton, bool); 5] {
        let options = &self.slack_search_options;
        [
            (
                SlackSurfaceActionButton {
                    id: "slack-search-from",
                    label: if options.from.is_some() {
                        "From me"
                    } else {
                        "From"
                    },
                    action: SurfaceState::toggle_slack_search_from_self,
                },
                options.from.is_some(),
            ),
            (
                SlackSurfaceActionButton {
                    id: "slack-search-in",
                    label: if options.in_conversation.is_some() {
                        "In current"
                    } else {
                        "In"
                    },
                    action: SurfaceState::toggle_slack_search_in_current,
                },
                options.in_conversation.is_some(),
            ),
            (
                SlackSurfaceActionButton {
                    id: "slack-search-only-my-channels",
                    label: "Only my channels",
                    action: SurfaceState::toggle_slack_search_only_my_channels,
                },
                options.only_my_channels,
            ),
            (
                SlackSurfaceActionButton {
                    id: "slack-search-automations",
                    label: if options.include_automations {
                        "Include automations"
                    } else {
                        "Exclude automations"
                    },
                    action: SurfaceState::toggle_slack_search_automations,
                },
                !options.include_automations,
            ),
            (
                SlackSurfaceActionButton {
                    id: "slack-search-sort",
                    label: match options.sort {
                        SlackMessageSearchSort::Relevant => "Most relevant",
                        SlackMessageSearchSort::Recent => "Most recent",
                    },
                    action: SurfaceState::toggle_slack_search_sort,
                },
                options.sort == SlackMessageSearchSort::Recent,
            ),
        ]
    }

    fn render_slack_search_refinement(
        &self,
        button: SlackSurfaceActionButton,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackSurfaceActionButton { id, label, action } = button;
        let palette = slack_palette(self.appearance_mode);
        let overlay_palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .h(px(30.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(if selected {
                palette.link
            } else {
                palette.attachment_border
            }))
            .bg(rgb(if selected {
                overlay_palette.selected_bg
            } else {
                palette.composer_bg
            }))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(overlay_palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(overlay_palette.hover_bg)))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .text_color(rgb(palette.main_text))
            .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    action(this, cx);
                }
            }))
            .child(label)
    }

    fn render_slack_search_results_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        if self.slack_search_rows.is_empty() {
            let message = if self.slack_search_loading {
                "Searching Slack…"
            } else {
                self.slack_search_error
                    .as_deref()
                    .unwrap_or("No messages matched this query.")
            };
            return div()
                .flex_grow(1.0)
                .min_h(px(0.0))
                .px(px(SLACK_SEARCH_RESULTS_HORIZONTAL_INSET))
                .pt(px(12.0))
                .text_size(px(15.0))
                .text_color(rgb(palette.main_secondary_text))
                .child(message.to_string())
                .into_any_element();
        }
        let view = cx.entity();
        list(
            self.slack_search_list_state.clone(),
            move |index, _window, cx| {
                view.update(cx, |this, cx| {
                    let row = this
                        .slack_search_rows
                        .get(index)
                        .expect("Slack search result row index must exist");
                    this.render_slack_search_result_row(row, cx)
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .flex_grow(1.0)
        .min_h(px(0.0))
        .into_any_element()
    }

    fn render_slack_search_result_row(
        &self,
        row: &SlackSearchRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let target = row.action_target.clone();
        div()
            .h(px(row.card_height + 8.0))
            .w_full()
            .flex_none()
            .px(px(SLACK_SEARCH_RESULTS_HORIZONTAL_INSET))
            .pb(px(8.0))
            .child(
                div()
                    .id(row.result_element_id.clone())
                    .role(Role::Group)
                    .aria_label(row.accessibility_label.clone())
                    .size_full()
                    .relative()
                    .px(px(16.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(rgb(palette.attachment_border))
                    .bg(rgb(palette.composer_bg))
                    .flex()
                    .flex_col()
                    .child(self.render_slack_search_result_message_action(row, target.clone(), cx))
                    .children(row.attachments.iter().map(|attachment| {
                        self.render_slack_search_attachment(
                            attachment,
                            row.action_target.clone(),
                            cx,
                        )
                    }))
                    .child(self.render_slack_search_reactions(row, cx))
                    .when(row.show_thread_action, |this| {
                        this.child(self.render_slack_search_thread_action(row, target, cx))
                    }),
            )
            .into_any_element()
    }

    fn render_slack_search_result_message_action(
        &self,
        row: &SlackSearchRow,
        target: StdArc<SlackMessageActionTarget>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let overlay_palette = slack_search_overlay_palette(self.appearance_mode);
        let keyboard_target = target.clone();
        div()
            .id(format!("{}-open-message", row.result_element_id))
            .role(Role::Button)
            .aria_label(row.accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .w_full()
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(overlay_palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(overlay_palette.hover_bg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_search_result(&target, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_search_result(&keyboard_target, cx);
                }
            }))
            .child(self.render_slack_search_result_message(row))
    }

    fn render_slack_search_result_message(&self, row: &SlackSearchRow) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(64.0))
            .flex_none()
            .py(px(10.0))
            .flex()
            .gap(px(8.0))
            .child(self.render_slack_search_avatar(row, 36.0))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .child(slack_search_result_metadata(row, &palette))
                    .child(
                        div()
                            .h(px(24.0))
                            .flex_none()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(self.styled_slack_search_body(row, 15.0, 22.0)),
                    ),
            )
    }

    fn render_slack_search_avatar(&self, row: &SlackSearchRow, size: f32) -> Div {
        let image = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        div()
            .size(px(size))
            .flex_none()
            .rounded(slack_base_icon_radius(size))
            .overflow_hidden()
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .when(image.is_none(), |this| {
                this.text_size(px(size * 0.36))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(row.avatar_text.clone())
            })
            .when_some(image, |this, image| {
                this.child(
                    img(image)
                        .size(px(size))
                        .rounded(slack_base_icon_radius(size)),
                )
            })
    }

    fn render_slack_search_reactions(&self, row: &SlackSearchRow, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(32.0))
            .flex_none()
            .relative()
            .ml(px(44.0))
            .mt(px(4.0))
            .child(self.render_slack_reactions(
                SlackReactionBarInput::new(
                    row.result_element_id.as_ref(),
                    Some(row.action_target.clone()),
                    row.reaction_state.clone(),
                    &row.reactions,
                    SlackMessageRenderContext::Search,
                ),
                cx,
            ))
    }
}
