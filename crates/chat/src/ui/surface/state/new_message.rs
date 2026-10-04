mod drafts;
mod lifecycle;
mod opening;
mod selection;

use std::{rc::Rc, sync::Arc};

use gpui::{Entity, ScrollStrategy, Window};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SlackMainTab, SlackRailView, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    normalize_slack_dm_finder_text, prepare_slack_destination_directory,
    slack_new_message_draft_key_for_conversation, slack_new_message_draft_key_for_people,
    slack_palette, SlackMainRoute, SlackNewMessageDestination, SlackNewMessageOpenLoad,
    SlackNewMessagePerson,
};
use crate::ui::{
    alpha, px, rgb, SlackConversationKind, SlackConversationOpenReceipt,
    SlackConversationOpenRequest, SlackDestinationTarget,
};

const SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS: usize = 14;

impl SurfaceState {
    pub(crate) fn initialize_slack_new_message_input(&mut self, cx: &mut Context<Self>) {
        let props = self.slack_new_message_input_props(cx);
        self.slack_new_message_to_input
            .update(cx, |input, cx| input.apply_props(props, cx));
    }

    pub(crate) fn slack_new_message_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let props = self.slack_new_message_input_props(cx);
        self.slack_new_message_to_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_new_message_to_input.clone()
    }

    fn slack_new_message_input_props(&self, cx: &mut Context<Self>) -> TextInputProps {
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |query, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_new_message_query(query, cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_submit: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.activate_selected_slack_new_message_candidate(cx);
                })
                .ok();
        });
        let on_escape = Self::slack_new_message_escape_action(cx);
        let on_up = Self::slack_new_message_move_action(cx, -1);
        let on_down = Self::slack_new_message_move_action(cx, 1);
        let on_focus = Self::slack_new_message_focus_action(cx);

        TextInputProps::single_line(self.slack_new_message_query.clone())
            .placeholder(
                if self.slack_new_message_selected_people.is_empty()
                    && self.slack_new_message_destination.is_none()
                {
                    "#a-channel, @somebody, or somebody@example.com"
                } else {
                    ""
                },
            )
            .style(self.slack_new_message_input_style())
            .bordered(false)
            .request_focus(
                self.slack_main_route == SlackMainRoute::NewMessage
                    && self.slack_new_message_to_focused,
            )
            .accessibility(
                self.slack_new_message_to_accessibility_id.clone(),
                "New message recipients",
            )
            .on_change(on_change)
            .on_submit(on_submit)
            .on_escape(on_escape)
            .on_up(on_up)
            .on_down(on_down)
            .on_focus(on_focus)
    }

    fn slack_new_message_escape_action(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    if surface.slack_new_message_query.is_empty() {
                        surface.leave_slack_new_message(cx);
                        Some(surface.focus_handle.clone())
                    } else {
                        surface.clear_slack_new_message_query(cx);
                        None
                    }
                })
                .ok()
                .flatten();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    fn slack_new_message_move_action(cx: &mut Context<Self>, direction: i32) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.move_slack_new_message_selection(direction, cx);
                })
                .ok();
        })
    }

    fn slack_new_message_focus_action(cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if surface.slack_main_route == SlackMainRoute::NewMessage {
                        surface.slack_new_message_to_focused = true;
                        surface.rebuild_slack_new_message_results();
                        surface.queue_slack_new_message_visible_images(
                            0,
                            SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS,
                            cx,
                        );
                        cx.notify();
                    }
                })
                .ok();
        })
    }

    fn slack_new_message_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(32.0),
            min_height: px(32.0),
            padding_x: px(2.0),
            padding_y: px(5.0),
            radius: px(0.0),
            background: alpha(0x000000, 0.0),
            border: alpha(0x000000, 0.0),
            focused_border: alpha(0x000000, 0.0),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.composer_placeholder).into(),
            selection: alpha(0x1264a3, 0.28),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(22.0),
            font_family: Some("Lato".into()),
        }
    }
}

fn next_slack_new_message_generation(generation: u64) -> u64 {
    generation
        .checked_add(1)
        .expect("Slack New message generation overflowed")
}
