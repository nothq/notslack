use super::super::super::{
    alpha, div, img, px, rgb, slack_base_icon_radius, slack_icon, Arc, Context, Div, FontWeight,
    Image, ParentElement, SlackShellIcon, Styled, SurfaceState,
};
use crate::ui::{SlackUserPresence, SlackWorkspace};

impl SurfaceState {
    pub(super) fn render_slack_workspace_badge(&self, workspace: &SlackWorkspace) -> Div {
        if let Some(image) = workspace
            .workspace_logo_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return render_slack_workspace_logo_badge(image);
        }
        div().size(px(36.0)).rounded(px(12.0))
    }

    pub(super) fn render_slack_self_badge(&self) -> Div {
        let (_, _, avatar_label, avatar_image_url) = self.slack_self_identity();
        if let Some(image) = avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return render_slack_workspace_logo_badge(image);
        }
        div()
            .size(px(36.0))
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(0x2e3136))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(avatar_label.unwrap_or_else(|| "IT".to_string()))
    }

    pub(super) fn render_slack_rail_self_presence(
        &self,
        presence: SlackUserPresence,
        notifications_paused: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let (icon, fill) = slack_rail_presence_icon(presence, notifications_paused);
        div()
            .absolute()
            .right(px(-6.0))
            .bottom(px(-6.0))
            .size(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_rail_presence_backplate())
            .child(slack_icon(icon, fill, 20.0, cx))
    }

    pub(super) fn slack_rail_item_badge_value(
        &self,
        workspace: &SlackWorkspace,
        label: &str,
    ) -> Option<String> {
        let (dms, _) = self.slack_conversation_read_overlay_dm_badges(workspace);
        let count = match label {
            "Home" => workspace.rail_badges.home,
            "DMs" => dms,
            "Activity" => workspace.rail_badges.activity,
            "Files" => workspace.rail_badges.files,
            "Later" => workspace.rail_badges.later,
            "More" => workspace.rail_badges.more,
            "Admin" => workspace.rail_badges.admin_attention.then_some(1),
            _ => None,
        }?;
        (count > 0).then(|| count.to_string())
    }

    pub(super) fn render_slack_rail_item_badge(&self, label: &str, value: &str) -> Div {
        if matches!(label, "Home" | "Admin") {
            return div()
                .absolute()
                .top(px(4.0))
                .right(px(4.0))
                .size(px(6.0))
                .rounded_full()
                .bg(rgb(0xef3e75));
        }
        div()
            .absolute()
            .top(px(-4.0))
            .right(px(-6.0))
            .pl(px(4.0))
            .pr(px(5.0))
            .h(px(16.0))
            .rounded_full()
            .bg(slack_rail_badge_background(label))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.0))
            .line_height(px(10.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if label == "Activity" {
                0xfff4f4
            } else {
                0xffffff
            }))
            .child(value.to_string())
    }
}

fn render_slack_workspace_logo_badge(image: Arc<Image>) -> Div {
    div()
        .size(px(36.0))
        .rounded(slack_base_icon_radius(36.0))
        .overflow_hidden()
        .child(
            img(image)
                .w_full()
                .h_full()
                .rounded(slack_base_icon_radius(36.0)),
        )
}

fn slack_rail_presence_icon(
    presence: SlackUserPresence,
    notifications_paused: bool,
) -> (SlackShellIcon, u32) {
    match (presence, notifications_paused) {
        (SlackUserPresence::Active, false) => (SlackShellIcon::PresenceActive, 0x20a271),
        (SlackUserPresence::Away, false) => (SlackShellIcon::PresenceAway, 0xf8f8f8),
        (SlackUserPresence::Active, true) => (SlackShellIcon::PresenceDndFilled, 0x20a271),
        (SlackUserPresence::Away, true) => (SlackShellIcon::PresenceDnd, 0xf8f8f8),
    }
}

fn slack_rail_presence_backplate() -> Div {
    div()
        .absolute()
        .left(px(2.5))
        .top(px(2.5))
        .size(px(15.0))
        .rounded_full()
        .bg(rgb(0x0e0e0e))
}

fn slack_rail_badge_background(label: &str) -> gpui::Hsla {
    if label == "Activity" {
        alpha(0xef3e75, 1.0)
    } else {
        alpha(0xf6f6f6, 0.25)
    }
}
