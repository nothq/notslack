use super::{
    alpha, div, list, px, rgb, slack_empty_messages, slack_icon, slack_palette, AnyElement,
    AppearanceMode, Context, FluentBuilder, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ListSizingBehavior, ParentElement, SlackMainTab, SlackRailView, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::{SlackMessageRenderContext, SlackMessageRow};
use crate::ui::SlackWorkspace;
use gpui::{ListOffset, Pixels, Role};
use gpui_components::selectable_text::SelectableTextDocument;

const SLACK_DATE_PILL_HEIGHT: f32 = 28.0;
const SLACK_DATE_PILL_PUSH_GAP: f32 = 5.0;
const SLACK_MESSAGES_SURFACE_TOP_PADDING: f32 = 18.0;
const SLACK_MESSAGES_PILL_OVERLAY_TOP: f32 = 6.0;

mod pending;

#[derive(Clone, Copy, Debug, PartialEq)]
struct SlackStickyDatePillLayout {
    row_index: usize,
    overlay_top: Pixels,
}

impl SurfaceState {
    pub(crate) fn render_slack_main_body(
        &self,
        workspace: &SlackWorkspace,
        view: gpui::Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.slack_active_rail_view {
            SlackRailView::Home | SlackRailView::Dms => match self.slack_active_tab {
                SlackMainTab::Messages => self.render_slack_messages_panel(workspace, view, cx),
                SlackMainTab::Canvas => self.render_slack_messages_panel(workspace, view, cx),
                SlackMainTab::BookmarkFolder => self.render_slack_bookmark_folder_panel(cx),
                SlackMainTab::Pins => self.render_slack_pins_panel(cx),
                SlackMainTab::FilesLinks => self.render_slack_conversation_files_panel(cx),
            },
            SlackRailView::Activity
            | SlackRailView::Files
            | SlackRailView::Later
            | SlackRailView::DraftsSent
            | SlackRailView::More
            | SlackRailView::Admin => div().into_any_element(),
        }
    }

    fn render_slack_messages_panel(
        &self,
        workspace: &SlackWorkspace,
        view: gpui::Entity<Self>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.is_loading_pending_slack_conversation(workspace) {
            return self.render_pending_slack_messages_panel();
        }
        self.render_slack_messages_surface(workspace, view, cx)
            .into_any_element()
    }

    fn render_slack_messages_surface(
        &self,
        workspace: &SlackWorkspace,
        view: gpui::Entity<Self>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let new_messages_pill = self.render_slack_new_messages_pill(cx);
        let sticky_date_layout = self.current_slack_sticky_date_pill_layout();
        let sticky_date_pill = sticky_date_layout.and_then(|layout| {
            self.render_slack_sticky_date_pill(layout.row_index, cx)
                .map(|pill| (pill, layout.overlay_top))
        });
        let sticky_date_surface = view.downgrade();
        let surface = cx.entity();
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .relative()
            .pt(px(SLACK_MESSAGES_SURFACE_TOP_PADDING))
            .pb(px(8.0))
            .on_children_prepainted(move |_, window, cx| {
                let should_refresh = sticky_date_surface
                    .update(cx, |surface, _| {
                        surface.current_slack_sticky_date_pill_layout() != sticky_date_layout
                    })
                    .unwrap_or(false);
                if should_refresh {
                    window.refresh();
                }
            })
            .on_scroll_wheel(move |_, _, cx| {
                surface.update(cx, |surface, _| {
                    surface.release_slack_conversation_unread_hold_for_reposition();
                    surface.mark_slack_remote_image_queue_dirty();
                });
            })
            .when(self.slack_message_display_row_count() == 0, |this| {
                this.child(
                    div()
                        .size_full()
                        .px(px(20.0))
                        .child(slack_empty_messages(self.appearance_mode)),
                )
            })
            .when(self.slack_message_display_row_count() > 0, |this| {
                this.child(self.render_slack_messages_list(view, workspace))
            })
            .when_some(sticky_date_pill, |this, (sticky_date_pill, overlay_top)| {
                this.child(self.render_slack_messages_pill_overlay(sticky_date_pill, overlay_top))
            })
            .when_some(new_messages_pill, |this, new_messages_pill| {
                this.child(self.render_slack_messages_pill_overlay(
                    new_messages_pill,
                    px(SLACK_MESSAGES_PILL_OVERLAY_TOP),
                ))
            })
    }

    fn render_slack_messages_pill_overlay(
        &self,
        pill: gpui::Div,
        overlay_top: Pixels,
    ) -> gpui::Div {
        div()
            .absolute()
            .top(overlay_top)
            .left(px(0.0))
            .right(px(0.0))
            .h(px(SLACK_DATE_PILL_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .child(pill)
    }

    fn render_slack_new_messages_pill(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let count = self.slack_new_message_count;
        if count == 0 {
            return None;
        }
        let label = format!(
            "{count} new {}",
            if count == 1 { "message" } else { "messages" }
        );
        Some(
            div()
                .h(px(28.0))
                .rounded_full()
                .overflow_hidden()
                .bg(rgb(0x1264a3))
                .flex()
                .items_center()
                .text_color(rgb(0xffffff))
                .child(self.render_slack_jump_to_first_unread(label, cx))
                .child(self.render_slack_dismiss_new_messages(cx)),
        )
    }

    fn render_slack_jump_to_first_unread(
        &self,
        label: String,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-jump-to-first-unread")
            .role(Role::Button)
            .aria_label("Jump to first unread message")
            .focusable()
            .tab_stop(true)
            .h_full()
            .pl(px(12.0))
            .pr(px(10.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .focus_visible(|style| style.bg(rgb(0x0b4c8c)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.jump_to_first_unread_slack_message(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.jump_to_first_unread_slack_message(cx);
            }))
            .child(slack_icon(SlackShellIcon::ArrowDown, 0xffffff, 14.0, cx))
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .child(label),
            )
    }

    fn render_slack_dismiss_new_messages(
        &self,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-dismiss-new-messages")
            .role(Role::Button)
            .aria_label("Dismiss new messages")
            .focusable()
            .tab_stop(true)
            .w(px(25.0))
            .h_full()
            .border_l_1()
            .border_color(alpha(0xffffff, 0.22))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .focus_visible(|style| style.bg(rgb(0x0b4c8c)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.dismiss_slack_new_messages(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.dismiss_slack_new_messages(cx);
            }))
            .child(slack_icon(SlackShellIcon::Close, 0xffffff, 12.0, cx))
    }

    fn current_slack_sticky_date_pill_layout(&self) -> Option<SlackStickyDatePillLayout> {
        let row_index = slack_sticky_date_row_index(
            &self.slack_message_rows,
            self.slack_message_list_state.logical_scroll_top(),
            self.slack_new_message_count,
        )?;
        let overlay_top = px(SLACK_MESSAGES_PILL_OVERLAY_TOP);
        let viewport_bounds = self.slack_message_list_state.viewport_bounds();
        if viewport_bounds.size.height <= px(0.0) {
            return Some(SlackStickyDatePillLayout {
                row_index,
                overlay_top,
            });
        }
        let overlay_window_top = viewport_bounds.top()
            - px(SLACK_MESSAGES_SURFACE_TOP_PADDING - SLACK_MESSAGES_PILL_OVERLAY_TOP);
        let next_divider_top = ((row_index + 1)..self.slack_message_rows.len())
            .map_while(|index| {
                self.slack_message_list_state
                    .bounds_for_item(index)
                    .map(|bounds| (index, bounds))
            })
            .take_while(|(_, bounds)| bounds.top() < viewport_bounds.bottom())
            .find_map(|(index, bounds)| {
                self.slack_message_rows[index]
                    .divider
                    .is_some()
                    .then_some(bounds.top())
            });
        let push_offset = next_divider_top
            .map(|next_divider_top| {
                slack_sticky_date_push_offset(next_divider_top, overlay_window_top)
            })
            .unwrap_or(px(0.0));
        Some(SlackStickyDatePillLayout {
            row_index,
            overlay_top: overlay_top + push_offset,
        })
    }

    fn render_slack_sticky_date_pill(
        &self,
        row_index: usize,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let label = self.slack_message_rows.get(row_index)?.date_label.clone()?;
        let palette = slack_palette(self.appearance_mode);
        let light_mode = self.appearance_mode == AppearanceMode::Light;
        Some(
            div()
                .h(px(SLACK_DATE_PILL_HEIGHT))
                .pl(px(16.0))
                .pr(px(8.0))
                .rounded(px(24.0))
                .bg(rgb(if light_mode {
                    palette.main_bg
                } else {
                    0x1a1d21
                }))
                .when(light_mode, |this| {
                    this.border_1().border_color(rgb(palette.main_border))
                })
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.date_divider_text))
                .child(label)
                .child(slack_icon(
                    SlackShellIcon::ChevronDown,
                    palette.date_divider_text,
                    12.0,
                    cx,
                )),
        )
    }

    fn is_loading_pending_slack_conversation(&self, workspace: &SlackWorkspace) -> bool {
        self.slack_pending_conversation_id
            .as_deref()
            .is_some_and(|conversation_id| conversation_id != workspace.conversation_id)
    }

    fn render_slack_messages_list(
        &self,
        view: gpui::Entity<Self>,
        workspace: &SlackWorkspace,
    ) -> AnyElement {
        let selection_document_id: gpui::SharedString = format!(
            "slack-selection:{}:{}:conversation:{}",
            workspace.team_id, workspace.conversation_id, self.slack_conversation_revision
        )
        .into();
        let row_document_id = selection_document_id.clone();
        let list_state = self.slack_message_list_state.clone();
        let messages = list(
            self.slack_message_list_state.clone(),
            move |index, _window, cx| {
                view.update(cx, |this, cx| {
                    let row = if index < this.slack_message_rows.len() {
                        &this.slack_message_rows[index]
                    } else {
                        this.slack_local_delivery_rows
                            .get(index - this.slack_message_rows.len())
                            .expect("Slack local delivery row index should exist")
                    };
                    this.render_slack_message_in_document(
                        row,
                        SlackMessageRenderContext::Conversation,
                        crate::ui::surface::SlackMessageDocumentPosition::row(
                            row_document_id.clone(),
                            index,
                        ),
                        cx,
                    )
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full();
        div()
            .id("slack-message-list")
            .role(Role::List)
            .aria_label(format!("Messages in {}", workspace.channel_name))
            .size_full()
            .child(
                SelectableTextDocument::new(
                    "slack-conversation-selection-document",
                    selection_document_id,
                    messages,
                )
                .on_autoscroll(move |distance, window, _cx| {
                    list_state.scroll_by(distance);
                    window.refresh();
                }),
            )
            .into_any_element()
    }
}

fn slack_sticky_date_row_index(
    rows: &[SlackMessageRow],
    scroll_top: ListOffset,
    new_message_count: usize,
) -> Option<usize> {
    if new_message_count > 0 {
        return None;
    }
    let row = rows.get(scroll_top.item_ix)?;
    if row.divider.is_some() && scroll_top.offset_in_item < px(SLACK_DATE_PILL_HEIGHT) {
        return None;
    }
    row.date_label.as_ref()?;
    Some(scroll_top.item_ix)
}

fn slack_sticky_date_push_offset(next_divider_top: Pixels, overlay_top: Pixels) -> Pixels {
    let push_distance = SLACK_DATE_PILL_HEIGHT + SLACK_DATE_PILL_PUSH_GAP
        - (next_divider_top - overlay_top).as_f32();
    px((-push_distance).clamp(-(SLACK_DATE_PILL_HEIGHT + SLACK_DATE_PILL_PUSH_GAP), 0.0))
}

#[cfg(test)]
mod tests;
