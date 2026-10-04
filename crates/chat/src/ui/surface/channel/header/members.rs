use crate::ui::surface::{
    slack_icon, slack_palette, SlackHeaderControl, SlackPalette, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    div, px, rgb, Context, Div, FontWeight, InteractiveElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use crate::ui::{AppearanceMode, SlackWorkspace};
use gpui::{Role, Stateful};

use super::slack_header_action_key;

pub(super) struct SlackMembersHeaderData {
    count: u32,
    accessibility_label: String,
}

impl SurfaceState {
    pub(super) fn slack_members_header_data(
        &self,
        workspace: &SlackWorkspace,
    ) -> Option<SlackMembersHeaderData> {
        if !self.slack_channel_members_control_available(workspace) {
            return None;
        }
        let snapshot = self.slack_members_snapshot.as_ref().filter(|snapshot| {
            snapshot.team_id == workspace.team_id
                && snapshot.conversation_id == workspace.conversation_id
        });
        let count = self.slack_effective_member_count(workspace)?;
        let included_names = snapshot
            .into_iter()
            .flat_map(|snapshot| snapshot.members.iter())
            .take(3)
            .map(|member| member.real_name.as_str())
            .collect::<Vec<_>>();
        let accessibility_label = match included_names.as_slice() {
            [] => format!("View all {count} members."),
            [name] => format!("View all {count} members. Includes {name}."),
            [first, second] => {
                format!("View all {count} members. Includes {first} and {second}.")
            }
            [first, second, third, ..] => {
                format!("View all {count} members. Includes {first}, {second}, and {third}.")
            }
        };
        Some(SlackMembersHeaderData {
            count,
            accessibility_label,
        })
    }

    pub(super) fn render_slack_members_header_button(
        &self,
        data: SlackMembersHeaderData,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let workspace = self
            .slack_workspace()
            .expect("Slack members header requires an active workspace");
        let tab_stop =
            self.slack_header_active_control(workspace) == Some(SlackHeaderControl::Members);
        self.slack_members_header_button_base(data, tab_stop, cx)
            .on_click(cx.listener(|this, _, _, cx| {
                this.slack_header_roving_target = SlackHeaderControl::Members;
                this.open_slack_members_panel(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if this.handle_slack_channel_header_roving_key(
                    SlackHeaderControl::Members,
                    event,
                    window,
                    cx,
                ) {
                    return;
                }
                if slack_header_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_members_panel(cx);
                }
            }))
    }

    fn slack_members_header_button_base(
        &self,
        data: SlackMembersHeaderData,
        tab_stop: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let border = match self.appearance_mode {
            AppearanceMode::Dark => 0x34363b,
            AppearanceMode::Light => palette.main_border,
        };
        div()
            .id("slack-channel-members")
            .role(Role::Button)
            .aria_label(data.accessibility_label)
            .aria_expanded(self.slack_members_panel_open)
            .track_focus(&self.slack_header_members_focus_handle)
            .focusable()
            .tab_stop(tab_stop)
            .w(px(49.0))
            .h(px(28.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(border))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(slack_header_button_hover(self.appearance_mode))))
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .child(slack_icon(
                SlackShellIcon::People,
                palette.main_secondary_text,
                20.0,
                cx,
            ))
            .child(slack_members_header_count(data.count, palette))
    }
}

fn slack_members_header_count(count: u32, palette: SlackPalette) -> Div {
    div()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(palette.main_text))
        .child(count.to_string())
}

fn slack_header_button_hover(appearance_mode: AppearanceMode) -> u32 {
    match appearance_mode {
        AppearanceMode::Dark => 0x2a2d31,
        AppearanceMode::Light => 0xf1f2f3,
    }
}
