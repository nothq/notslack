use gpui::Role;

use super::{
    div, img, px, rgb, slack_base_icon_radius, AnyElement, AppearanceMode, Context, Div,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SlackMainTab,
    SlackRailView, StatefulInteractiveElement, Styled, SurfaceState, Window,
};
use crate::ui::{
    surface::{SlackExternalMembersSummary, SlackExternalOrganizationBadge},
    SlackWorkspace,
};

impl SurfaceState {
    pub(crate) fn render_slack_external_members_banner(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let summary = self.slack_external_members_summary_for_workspace(workspace)?;
        let organizations = summary.organizations.clone();
        let people_label = summary.people_label.clone();
        let organizations_label = summary.organizations_label.clone();
        let accessibility_label = summary.accessibility_label.clone();
        let (background, text, link) = match self.appearance_mode {
            AppearanceMode::Dark => (0x462408, 0xd1d2d3, 0x36c5f0),
            AppearanceMode::Light => (0xfff2e0, 0x1d1c1d, 0x1264a3),
        };
        Some(
            div()
                .id("slack-external-members-banner")
                .role(Role::Group)
                .aria_label(accessibility_label)
                .h(px(42.0))
                .flex_none()
                .mx(px(20.0))
                .px(px(12.0))
                .rounded_t(px(8.0))
                .bg(rgb(background))
                .overflow_hidden()
                .flex()
                .items_center()
                .text_size(px(13.0))
                .line_height(px(18.0))
                .text_color(rgb(text))
                .child(self.render_slack_external_organization_badges(&organizations))
                .child(self.render_slack_external_members_link(people_label, link, cx))
                .child(div().ml(px(4.0)).child(organizations_label))
                .into_any_element(),
        )
    }

    fn render_slack_external_organization_badges(
        &self,
        organizations: &[SlackExternalOrganizationBadge],
    ) -> Div {
        div()
            .flex_none()
            .flex()
            .items_center()
            .children(
                organizations
                    .iter()
                    .enumerate()
                    .map(|(index, organization)| {
                        self.render_slack_external_organization_badge(organization, index)
                    }),
            )
    }

    fn render_slack_external_members_link(
        &self,
        people_label: gpui::SharedString,
        link: u32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-external-members-link")
            .role(Role::Button)
            .aria_label(people_label.clone())
            .focusable()
            .tab_stop(true)
            .ml(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(link))
            .cursor_pointer()
            .child(people_label)
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_members_panel(cx);
            }))
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window: &mut Window, cx| {
                    if slack_external_members_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.open_slack_members_panel(cx);
                    }
                }),
            )
    }

    fn slack_external_members_summary_for_workspace(
        &self,
        workspace: &SlackWorkspace,
    ) -> Option<&SlackExternalMembersSummary> {
        if self.slack_main_route != crate::ui::surface::SlackMainRoute::Conversation
            || !matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
            || self.slack_active_tab != SlackMainTab::Messages
            || !self.slack_workspace_api_capabilities.send_message
            || !self
                .slack_workspace_api_capabilities
                .load_conversation_members
        {
            return None;
        }
        let snapshot = self.slack_members_snapshot.as_ref()?;
        let summary = self.slack_external_members_summary.as_ref()?;
        (snapshot.next_cursor.is_none()
            && snapshot.team_id == workspace.team_id
            && snapshot.conversation_id == workspace.conversation_id
            && self.slack_members_conversation_id.as_deref()
                == Some(workspace.conversation_id.as_str())
            && summary.team_id.as_ref() == workspace.team_id.as_str()
            && summary.conversation_id.as_ref() == workspace.conversation_id.as_str()
            && summary.external_member_count > 0
            && summary.external_organization_count > 0)
            .then_some(summary)
    }

    fn render_slack_external_organization_badge(
        &self,
        organization: &SlackExternalOrganizationBadge,
        index: usize,
    ) -> impl IntoElement {
        let badge = if let Some(image) = organization
            .image_url
            .as_ref()
            .and_then(|image_url| self.slack_remote_images.get(image_url.as_ref()))
            .cloned()
        {
            div()
                .size(px(16.0))
                .rounded(slack_base_icon_radius(16.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(16.0)))
        } else {
            div()
                .size(px(16.0))
                .rounded(slack_base_icon_radius(16.0))
                .bg(rgb(0xababad))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x1a1d21))
                .child(organization.initials.clone())
        };
        div()
            .id(("slack-connected-organization", index))
            .role(Role::Group)
            .aria_label(organization.name.clone())
            .ml(px(if index == 0 { 0.0 } else { -6.0 }))
            .size(px(16.0))
            .child(badge)
    }
}

fn slack_external_members_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
