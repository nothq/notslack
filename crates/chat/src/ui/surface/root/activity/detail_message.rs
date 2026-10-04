use crate::ui::surface::{
    alpha, slack_base_icon_radius, slack_icon, slack_message_body_block_in_document, slack_palette,
    SlackMessageDocumentPosition, SlackMessageRenderContext, SlackMessageRow,
    SlackMessageSelectionContext, SlackReactionPickerAnchor, SlackReactionPickerSource,
    SlackShellIcon, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, px, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Role, Stateful, StatefulInteractiveElement, Styled,
};
use std::{cell::Cell, sync::Arc};

use crate::ui::surface::SlackMessageActionTarget;

/// An Activity detail row in the selection document; each rendered message claims the next index.
#[derive(Clone, Copy)]
struct SlackActivityDetailDocumentRow<'a> {
    document_id: &'a gpui::SharedString,
    row_index: usize,
    next_message_index: &'a Cell<usize>,
}

struct SlackActivityDetailReactionBinding {
    target: Arc<SlackMessageActionTarget>,
    reactions: Arc<[crate::ui::SlackReaction]>,
    pending: bool,
}

impl SurfaceState {
    pub(super) fn render_slack_activity_detail_message_in_document(
        &self,
        row: &SlackMessageRow,
        anchor_timestamp: &str,
        position: SlackMessageDocumentPosition,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackMessageDocumentPosition {
            document_id,
            row_index,
            message_index,
        } = position;
        self.render_slack_activity_detail_message_at_document_position(
            row,
            anchor_timestamp,
            SlackActivityDetailDocumentRow {
                document_id: &document_id,
                row_index,
                next_message_index: &Cell::new(message_index),
            },
            cx,
        )
    }

    fn render_slack_activity_detail_message_at_document_position(
        &self,
        row: &SlackMessageRow,
        anchor_timestamp: &str,
        document_row: SlackActivityDetailDocumentRow<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let highlighted = row.id == anchor_timestamp;
        let palette = slack_palette(self.appearance_mode);
        let hover_group = format!("slack-activity-detail-message-hover-{}", row.id);
        let selection_context = slack_activity_detail_selection_context(row, document_row);
        let replies = self.render_slack_activity_detail_message_replies(
            row,
            anchor_timestamp,
            document_row,
            cx,
        );
        div()
            .w_full()
            .min_w(px(0.0))
            .relative()
            .group(hover_group.clone())
            .px(px(20.0))
            .py(px(8.0))
            .when(highlighted, |this| this.bg(rgb(palette.topic_bg)))
            .when_some(row.divider.as_ref(), |this, divider| {
                this.child(slack_activity_detail_divider(
                    divider.label.clone(),
                    palette.main_secondary_text,
                ))
            })
            .child(self.render_slack_activity_detail_message_row(row, &selection_context, cx))
            .when(
                self.slack_workspace_api_capabilities.mutate_reactions
                    && row.action_target.is_some(),
                |this| {
                    this.child(self.render_slack_activity_detail_reaction_action(
                        row,
                        &hover_group,
                        cx,
                    ))
                },
            )
            .children(replies)
            .into_any_element()
    }

    fn render_slack_activity_detail_message_row(
        &self,
        row: &SlackMessageRow,
        selection_context: &SlackMessageSelectionContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w_full()
            .flex()
            .items_start()
            .gap(px(9.0))
            .child(self.render_slack_activity_detail_avatar(row))
            .child(self.render_slack_activity_detail_message_content(row, selection_context, cx))
    }

    fn render_slack_activity_detail_message_replies(
        &self,
        row: &SlackMessageRow,
        anchor_timestamp: &str,
        document_row: SlackActivityDetailDocumentRow<'_>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        row.replies
            .iter()
            .map(|reply| {
                self.render_slack_activity_detail_message_at_document_position(
                    reply,
                    anchor_timestamp,
                    document_row,
                    cx,
                )
            })
            .collect()
    }

    fn render_slack_activity_detail_reaction_action(
        &self,
        row: &SlackMessageRow,
        hover_group: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let picker = self.slack_reaction_picker.as_ref().filter(|picker| {
            row.action_target.as_ref().is_some_and(|target| {
                picker.identity.target() == target
                    && picker.identity.render_context() == SlackMessageRenderContext::Activity
                    && picker.identity.anchor() == SlackReactionPickerAnchor::HoverAction
            })
        });
        let persistent = picker.is_some();
        let pending = self.slack_reaction_pending_for_target(
            row.action_target
                .as_deref()
                .expect("supported Activity reaction action must have a typed target"),
        );
        div()
            .id(format!("slack-activity-detail-actions-{}", row.id))
            .role(Role::Group)
            .aria_label("Message actions")
            .absolute()
            .right(px(16.0))
            .top(px(0.0))
            .h(px(40.0))
            .p(px(4.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(palette.main_secondary_text, 0.13))
            .bg(rgb(palette.main_bg))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
            .focus_visible(|style| style.opacity(1.0))
            .child(self.render_slack_activity_detail_reaction_button(row, pending, cx))
            .when_some(picker, |this, picker| {
                this.child(self.render_slack_reaction_picker(picker, cx))
            })
    }

    fn render_slack_activity_detail_reaction_button(
        &self,
        row: &SlackMessageRow,
        pending: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let target = row
            .action_target
            .clone()
            .expect("supported Activity reaction action must have a typed target");
        let reactions = row.reaction_state.clone();
        let button = div()
            .id(format!("slack-activity-detail-add-reaction-{}", row.id))
            .role(Role::Button)
            .aria_label(activity_reaction_action_label(pending))
            .focusable()
            .tab_stop(true)
            .size(px(32.0))
            .rounded(px(8.0))
            .focus_visible(move |style| style.bg(alpha(palette.main_secondary_text, 0.10)));
        self.bind_slack_activity_detail_reaction_button(
            button,
            SlackActivityDetailReactionBinding {
                target,
                reactions,
                pending,
            },
            cx,
        )
        .flex()
        .items_center()
        .justify_center()
        .child(slack_icon(
            SlackShellIcon::AddReaction,
            palette.main_secondary_text,
            18.0,
            cx,
        ))
    }

    fn bind_slack_activity_detail_reaction_button(
        &self,
        button: Stateful<Div>,
        binding: SlackActivityDetailReactionBinding,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let SlackActivityDetailReactionBinding {
            target,
            reactions,
            pending,
        } = binding;
        let keyboard_target = target.clone();
        let keyboard_reactions = reactions.clone();
        button.when(!pending, |this| {
            this.cursor_pointer()
                .hover(move |style| style.bg(alpha(palette.main_secondary_text, 0.06)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_slack_reaction_picker_for_target(
                        activity_reaction_picker_source(target.clone(), reactions.clone()),
                        cx,
                    );
                }))
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if activity_reaction_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_reaction_picker_for_target(
                            activity_reaction_picker_source(
                                keyboard_target.clone(),
                                keyboard_reactions.clone(),
                            ),
                            cx,
                        );
                    }
                }))
        })
    }

    fn render_slack_activity_detail_message_content(
        &self,
        row: &SlackMessageRow,
        selection_context: &SlackMessageSelectionContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .child(div().h(px(21.0)).child(self.render_slack_message_author(
                row,
                SlackMessageRenderContext::Activity,
                Some(selection_context),
                cx,
            )))
            .when(
                row.body.has_renderable_content() || !row.table_rows.is_empty(),
                |this| {
                    this.child(slack_message_body_block_in_document(
                        self,
                        row,
                        SlackMessageRenderContext::Activity,
                        selection_context,
                        cx,
                    ))
                },
            )
            .when(!row.attachments.is_empty(), |this| {
                this.child(self.render_slack_message_attachments(
                    row,
                    SlackMessageRenderContext::Activity,
                    cx,
                ))
            })
            .when(!row.reactions.is_empty(), |this| {
                this.child(self.render_slack_message_reactions(
                    row,
                    SlackMessageRenderContext::Activity,
                    cx,
                ))
            })
    }

    fn render_slack_activity_detail_avatar(&self, row: &SlackMessageRow) -> Div {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .size(px(36.0))
                .flex_none()
                .rounded(slack_base_icon_radius(36.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(36.0)));
        }
        div()
            .size(px(36.0))
            .flex_none()
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_text.clone())
    }
}

fn slack_activity_detail_selection_context(
    row: &SlackMessageRow,
    document_row: SlackActivityDetailDocumentRow<'_>,
) -> SlackMessageSelectionContext {
    let SlackActivityDetailDocumentRow {
        document_id: selection_document_id,
        row_index,
        next_message_index,
    } = document_row;
    let message_index = next_message_index.get();
    next_message_index.set(message_index + 1);
    SlackMessageSelectionContext::new(
        selection_document_id.clone(),
        row_index,
        message_index,
        row.id.clone(),
    )
}

fn activity_reaction_picker_source(
    target: Arc<SlackMessageActionTarget>,
    reactions: Arc<[crate::ui::SlackReaction]>,
) -> SlackReactionPickerSource {
    SlackReactionPickerSource::new(
        target,
        reactions,
        SlackMessageRenderContext::Activity,
        SlackReactionPickerAnchor::HoverAction,
    )
}

fn activity_reaction_action_label(pending: bool) -> &'static str {
    if pending {
        "Add reaction…, reaction change pending"
    } else {
        "Add reaction…"
    }
}

fn activity_reaction_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}

fn slack_activity_detail_divider(label: gpui::SharedString, color: u32) -> Div {
    div()
        .h(px(34.0))
        .pb(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(color))
        .child(label)
}
