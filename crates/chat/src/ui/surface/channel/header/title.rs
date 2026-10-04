use std::sync::Arc;

use crate::ui::surface::channel::{
    slack_avatar_presence_badge, SlackAvatarPresenceBadgeSize, SlackAvatarPresenceBadgeSpec,
};
use crate::ui::surface::{
    slack_base_icon_radius, slack_icon, slack_palette, SlackHeaderControl, SlackRailView,
    SlackShellIcon, SlackSidebarRowKind, SurfaceState,
};
use crate::ui::{
    div, img, px, rgb, AnyElement, Context, Div, FluentBuilder, FontWeight, Image,
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
};
use crate::ui::{initials, SlackConversationKind, SlackUserPresence, SlackWorkspace};
use gpui::{AppContext, Role};

use super::{controls::SlackHeaderTooltip, slack_header_action_key};

#[derive(Default)]
struct SlackDirectMessageHeaderData {
    profile_user_id: Option<String>,
    avatar_image: Option<Arc<Image>>,
    peer_presence: Option<SlackUserPresence>,
}

impl SurfaceState {
    pub(crate) fn render_slack_main_title(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = self.slack_main_title_label(workspace);
        match self.slack_active_rail_view {
            SlackRailView::Home | SlackRailView::Dms
                if matches!(
                    workspace.channel_kind,
                    SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
                ) =>
            {
                self.render_slack_direct_message_title(workspace, &title, cx)
            }
            SlackRailView::Home | SlackRailView::Dms => {
                self.render_slack_channel_title(workspace, &title, cx)
            }
            _ => self.render_slack_plain_main_title(&title),
        }
    }

    fn slack_main_title_label(&self, workspace: &SlackWorkspace) -> String {
        match self.slack_active_rail_view {
            SlackRailView::Home | SlackRailView::Dms => self
                .pending_slack_conversation_title(workspace)
                .unwrap_or_else(|| workspace.channel_name.clone()),
            view => view.title().to_string(),
        }
    }

    fn pending_slack_conversation_title(&self, workspace: &SlackWorkspace) -> Option<String> {
        let conversation_id = self.slack_pending_conversation_id.as_deref()?;
        if conversation_id == workspace.conversation_id {
            return None;
        }
        self.slack_sidebar_rows
            .iter()
            .find_map(|row| match &row.kind {
                SlackSidebarRowKind::Item { item, .. } if item.target_id == conversation_id => {
                    Some(item.label.clone())
                }
                _ => None,
            })
    }

    fn render_slack_direct_message_title(
        &self,
        workspace: &SlackWorkspace,
        title: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let header_data = self.slack_direct_message_header_data(workspace);
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .when(self.slack_workspace_api_capabilities.mutate_stars, |this| {
                this.child(self.render_slack_header_star_button(
                    "slack-direct-message-star",
                    "Star conversation",
                    cx,
                ))
            })
            .child(self.render_slack_direct_message_details(workspace, title, header_data, cx))
    }

    fn render_slack_direct_message_details(
        &self,
        workspace: &SlackWorkspace,
        title: &str,
        header_data: SlackDirectMessageHeaderData,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let details = div()
            .h(px(30.0))
            .pl(px(3.0))
            .pr(px(8.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.render_slack_direct_message_title_avatar(
                workspace,
                title,
                &header_data,
                cx,
            ))
            .child(self.render_slack_main_title_text(title));
        let Some(user_id) = header_data.profile_user_id else {
            return details.into_any_element();
        };
        let clicked_user_id = user_id.clone();
        details
            .id("slack-conversation-details")
            .role(Role::Button)
            .aria_label(format!("Conversation details for {title}"))
            .focusable()
            .tab_stop(true)
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x2a2d31)))
            .focus_visible(|style| style.bg(rgb(0x2a2d31)))
            .tooltip(move |_, cx| {
                cx.new(|_| SlackHeaderTooltip {
                    label: "View profile",
                })
                .into()
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_profile(&clicked_user_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_header_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_profile(&user_id, cx);
                }
            }))
            .into_any_element()
    }

    fn slack_direct_message_header_data(
        &self,
        workspace: &SlackWorkspace,
    ) -> SlackDirectMessageHeaderData {
        if workspace.channel_kind != SlackConversationKind::DirectMessage {
            return SlackDirectMessageHeaderData::default();
        }
        let Some(item) = workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .find(|item| item.target_id == workspace.conversation_id)
        else {
            return SlackDirectMessageHeaderData::default();
        };
        SlackDirectMessageHeaderData {
            profile_user_id: if self.slack_workspace_api_capabilities.load_profile {
                item.user_id.clone()
            } else {
                None
            },
            avatar_image: item
                .avatar_image_url
                .as_deref()
                .and_then(|url| self.slack_remote_images.get(url).cloned()),
            peer_presence: item.user_id.as_deref().and_then(|user_id| {
                self.slack_presence_authority.resolve_presence(
                    &workspace.team_id,
                    user_id,
                    item.presence,
                )
            }),
        }
    }

    fn render_slack_channel_title(
        &self,
        workspace: &SlackWorkspace,
        title: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .when(self.slack_channel_move_menu_available(workspace), |this| {
                this.child(self.render_slack_channel_move_button(workspace, cx))
            })
            .child(self.render_slack_channel_title_and_topic(workspace, title, cx))
    }

    fn render_slack_channel_title_and_topic(
        &self,
        workspace: &SlackWorkspace,
        title: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .gap(px(16.0))
            .child(self.render_slack_channel_title_control(workspace, title, cx))
            .when(!workspace.channel_topic.is_empty(), |this| {
                this.child(
                    div()
                        .flex_grow(1.0)
                        .min_w(px(0.0))
                        .max_w(px(520.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(15.0))
                        .line_height(px(18.0))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(rgb(palette.main_secondary_text))
                        .child(workspace.channel_topic.clone()),
                )
            })
    }

    fn render_slack_channel_title_control(
        &self,
        workspace: &SlackWorkspace,
        title: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let control = div()
            .h(px(28.0))
            .px(px(8.0))
            .flex_none()
            .max_w(px(420.0))
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(slack_channel_title_icon(workspace, palette.main_text, cx))
            .child(self.render_slack_main_title_text(title));
        if !self.slack_workspace_api_capabilities.load_channel_details {
            return control.into_any_element();
        }
        let tab_stop =
            self.slack_header_active_control(workspace) == Some(SlackHeaderControl::ChannelDetails);
        control
            .id("slack-conversation-details")
            .role(Role::Button)
            .aria_label(format!("Channel details for #{title}"))
            .track_focus(&self.slack_header_details_focus_handle)
            .focusable()
            .tab_stop(tab_stop)
            .rounded(px(8.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x2a2d31)))
            .focus_visible(|style| style.bg(rgb(0x2a2d31)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.slack_header_roving_target = SlackHeaderControl::ChannelDetails;
                this.open_slack_channel_details(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if this.handle_slack_channel_header_roving_key(
                    SlackHeaderControl::ChannelDetails,
                    event,
                    window,
                    cx,
                ) {
                    return;
                }
                if slack_header_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_channel_details(cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_plain_main_title(&self, title: &str) -> Div {
        div()
            .flex()
            .items_center()
            .child(self.render_slack_main_title_text(title))
    }

    fn render_slack_main_title_text(&self, title: &str) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .text_size(px(18.0))
            .font_weight(FontWeight::BLACK)
            .text_color(rgb(palette.main_text))
            .child(title.to_string())
    }

    fn render_slack_direct_message_title_avatar(
        &self,
        workspace: &SlackWorkspace,
        title: &str,
        header_data: &SlackDirectMessageHeaderData,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        if matches!(workspace.channel_kind, SlackConversationKind::GroupMessage) {
            return slack_icon(SlackShellIcon::People, palette.main_text, 20.0, cx);
        }
        let peer_presence = header_data.peer_presence;
        let avatar = if let Some(image) = header_data.avatar_image.clone() {
            div()
                .size(px(24.0))
                .rounded(slack_base_icon_radius(24.0))
                .overflow_hidden()
                .child(
                    img(image)
                        .w_full()
                        .h_full()
                        .rounded(slack_base_icon_radius(24.0)),
                )
        } else {
            div()
                .size(px(24.0))
                .rounded(slack_base_icon_radius(24.0))
                .bg(rgb(0x2f7ae5))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(initials(title))
        };
        div()
            .relative()
            .size(px(24.0))
            .child(avatar)
            .when_some(
                slack_avatar_presence_badge(
                    SlackAvatarPresenceBadgeSpec {
                        size: SlackAvatarPresenceBadgeSize::Header24,
                        presence: peer_presence,
                        notifications_paused: workspace.peer_notifications_paused,
                        background: palette.main_bg,
                        active_color: 0x20a271,
                        away_color: 0x616061,
                    },
                    cx,
                ),
                |this, badge| this.child(badge),
            )
            .into_any_element()
    }
}

fn slack_channel_title_icon(
    workspace: &SlackWorkspace,
    color: u32,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    slack_icon(
        if workspace.channel_kind == SlackConversationKind::PrivateChannel {
            SlackShellIcon::LockSmall
        } else {
            SlackShellIcon::ChannelFilled
        },
        color,
        18.0,
        cx,
    )
}
