use super::super::super::{
    alpha, div, img, px, rgb, slack_base_icon_radius, slack_icon, slack_palette, AnyElement,
    Context, Div, FluentBuilder, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SlackShellIcon, StatefulInteractiveElement, Styled,
    SurfaceState,
};
use super::slack_new_message_action_key;
use crate::ui::surface::SlackNewMessageDestination;
use crate::ui::SlackWorkspace;
use gpui::Role;

const SLACK_NEW_MESSAGE_TO_HEIGHT: f32 = 52.0;

impl SurfaceState {
    pub(super) fn render_slack_new_message_to_row(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let destination_resolved = self.slack_new_message_destination.is_some();
        let input_visible =
            !destination_resolved || !self.slack_new_message_selected_people.is_empty();
        div()
            .h(px(SLACK_NEW_MESSAGE_TO_HEIGHT))
            .flex_none()
            .px(px(20.0))
            .border_b_1()
            .border_color(rgb(palette.main_border))
            .flex()
            .items_center()
            .gap(px(10.0))
            .when(input_visible, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, window, cx| {
                        let focus = this
                            .slack_new_message_to_input
                            .read(cx)
                            .focus_handle_clone();
                        window.focus(&focus, cx);
                    }),
                )
            })
            .child(slack_new_message_to_label(palette.main_secondary_text))
            .child(self.render_slack_new_message_recipients(input_visible, cx))
            .when(destination_resolved, |this| {
                this.child(self.render_slack_new_message_destination_actions(workspace, cx))
            })
    }

    fn render_slack_new_message_recipients(
        &self,
        input_visible: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h(px(34.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .overflow_hidden()
            .children(
                self.slack_new_message_selected_people
                    .iter()
                    .map(|person| self.render_slack_new_message_person_chip(person, cx)),
            )
            .when(self.slack_new_message_selected_people.is_empty(), |this| {
                this.when_some(
                    self.slack_new_message_destination.as_ref(),
                    |this, destination| {
                        this.child(self.render_slack_new_message_destination_chip(destination, cx))
                    },
                )
            })
            .when(input_visible, |this| {
                this.child(
                    div()
                        .flex_grow(1.0)
                        .min_w(px(120.0))
                        .h(px(32.0))
                        .child(self.slack_new_message_input_entity(cx)),
                )
            })
    }

    fn render_slack_new_message_destination_chip(
        &self,
        destination: &SlackNewMessageDestination,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let label = destination.label.clone();
        let accessibility_label = if destination.kind.is_channel() {
            format!("#{label}")
        } else {
            label.to_string()
        };
        div()
            .id("slack-new-message-destination")
            .role(Role::Button)
            .aria_label(accessibility_label)
            .focusable()
            .tab_stop(true)
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h(px(22.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(6.0))
            .overflow_hidden()
            .child(slack_new_message_destination_icon(
                destination,
                palette.main_text,
                cx,
            ))
            .child(slack_new_message_destination_label(
                label,
                palette.main_text,
            ))
            .on_click(cx.listener(|this, _, window, cx| {
                this.edit_slack_new_message_destination(window, cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_new_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.edit_slack_new_message_destination(window, cx);
                }
            }))
            .into_any_element()
    }

    fn edit_slack_new_message_destination(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_slack_new_message_destination(cx);
        let focus = self
            .slack_new_message_to_input
            .read(cx)
            .focus_handle_clone();
        window.focus(&focus, cx);
    }

    fn render_slack_new_message_destination_actions(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(
                self.slack_workspace_api_capabilities.search_messages,
                |this| this.child(self.render_slack_new_message_search_button(cx)),
            )
            .when(
                workspace.channel_kind.is_channel()
                    && self
                        .slack_workspace_api_capabilities
                        .load_conversation_members
                    && workspace.member_count.is_some(),
                |this| {
                    this.child(self.render_slack_new_message_members_button(
                        workspace.member_count.expect("member count checked above"),
                        cx,
                    ))
                },
            )
    }

    fn render_slack_new_message_search_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-new-message-search")
            .role(Role::Button)
            .aria_label("Search in channel")
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(alpha(palette.main_text, 0.06)))
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .child(slack_icon(
                SlackShellIcon::Search,
                palette.main_secondary_text,
                20.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_conversation_search(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_new_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_conversation_search(cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_new_message_members_button(
        &self,
        member_count: u32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let has_avatars = !self.slack_members_rows.is_empty();
        div()
            .id("slack-new-message-members")
            .role(Role::Button)
            .aria_label(format!("View all {member_count} members."))
            .aria_expanded(self.slack_members_panel_open)
            .focusable()
            .tab_stop(true)
            .w(px(if has_avatars { 88.0 } else { 49.0 }))
            .h(px(28.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(alpha(palette.main_text, 0.06)))
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .when(has_avatars, |this| {
                this.child(self.render_slack_new_message_member_avatars(palette.main_bg))
            })
            .when(!has_avatars, |this| {
                this.child(slack_icon(
                    SlackShellIcon::People,
                    palette.main_secondary_text,
                    20.0,
                    cx,
                ))
            })
            .child(slack_new_message_member_count(
                member_count,
                palette.main_text,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_members_panel(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_new_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_members_panel(cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_new_message_member_avatars(&self, background: u32) -> Div {
        div().h(px(20.0)).flex().items_center().children(
            self.slack_members_rows
                .iter()
                .take(4)
                .enumerate()
                .map(|(index, member)| {
                    div()
                        .relative()
                        .ml(px(if index == 0 { 0.0 } else { -7.0 }))
                        .size(px(20.0))
                        .rounded(slack_base_icon_radius(20.0))
                        .border_1()
                        .border_color(rgb(background))
                        .overflow_hidden()
                        .child(self.render_slack_new_message_member_avatar(member))
                }),
        )
    }

    fn render_slack_new_message_member_avatar(
        &self,
        member: &crate::ui::surface::SlackMemberRow,
    ) -> Div {
        if let Some(image) = member
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .size_full()
                .child(img(image).size_full().rounded(slack_base_icon_radius(20.0)));
        }
        div()
            .size_full()
            .bg(rgb(member.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(7.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(member.avatar_initials.clone())
    }
}

fn slack_new_message_to_label(text_color: u32) -> Div {
    div()
        .flex_none()
        .text_size(px(15.0))
        .line_height(px(22.0))
        .text_color(rgb(text_color))
        .child("To:")
}

fn slack_new_message_destination_icon(
    destination: &SlackNewMessageDestination,
    text_color: u32,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let icon = if destination.kind.is_channel() {
        if destination.kind == crate::ui::SlackConversationKind::PrivateChannel {
            SlackShellIcon::LockSmall
        } else {
            SlackShellIcon::HashSmall
        }
    } else {
        SlackShellIcon::People
    };
    slack_icon(icon, text_color, 18.0, cx)
}

fn slack_new_message_destination_label(label: gpui::SharedString, text_color: u32) -> Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(15.0))
        .line_height(px(22.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(text_color))
        .child(label)
}

fn slack_new_message_member_count(member_count: u32, text_color: u32) -> Div {
    div()
        .ml(px(4.0))
        .text_size(px(13.0))
        .line_height(px(18.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(text_color))
        .child(member_count.to_string())
}
