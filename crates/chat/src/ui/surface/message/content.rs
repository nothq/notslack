use super::rows::{
    bind_slack_profile_click, slack_message_body_block, slack_message_body_block_in_document,
    SlackMessageSelectionContext,
};
use super::SLACK_MESSAGE_EDGE_PADDING;
use crate::ui::surface::{
    div, img, px, rgb, slack_base_icon_radius, slack_palette, AnyElement, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    SlackExternalOrganizationBadge, SlackMessageDeliveryAction, SlackMessageDeliveryState,
    SlackMessageRenderContext, SlackMessageRow, StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::Role;

mod author;

impl SurfaceState {
    pub(super) fn render_slack_message_avatar(
        &self,
        row: &SlackMessageRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let avatar_face = if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
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
        } else {
            div()
                .size(px(36.0))
                .rounded(slack_base_icon_radius(36.0))
                .bg(rgb(row.avatar_fill))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(row.avatar_text.clone())
        };
        let external_organization = row.user_id.as_deref().and_then(|user_id| {
            self.slack_external_organization_badges_by_user_id
                .get(user_id)
        });
        let avatar = div()
            .size(px(36.0))
            .relative()
            .child(avatar_face)
            .when_some(external_organization, |this, organization| {
                this.child(self.render_slack_message_external_organization_badge(organization))
            });
        bind_slack_profile_click(
            avatar,
            row.user_id.clone(),
            format!("slack-message-avatar-{}", row.id),
            format!("Open profile for {}", row.author),
            cx,
        )
    }

    fn render_slack_message_external_organization_badge(
        &self,
        organization: &SlackExternalOrganizationBadge,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let image = organization
            .image_url
            .as_ref()
            .and_then(|url| self.slack_remote_images.get(url.as_ref()))
            .cloned();
        div()
            .absolute()
            .right(px(-2.0))
            .bottom(px(-2.0))
            .size(px(16.0))
            .rounded(px(4.0))
            .border_2()
            .border_color(rgb(palette.main_bg))
            .overflow_hidden()
            .bg(rgb(0xababad))
            .flex()
            .items_center()
            .justify_center()
            .when(image.is_none(), |this| {
                this.text_size(px(7.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x1a1d21))
                    .child(organization.initials.clone())
            })
            .when_some(image, |this, image| {
                this.child(img(image).size_full().rounded(px(2.0)))
            })
    }

    pub(super) fn render_slack_message_content(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        selection_context: Option<&SlackMessageSelectionContext>,
        cx: &mut Context<Self>,
    ) -> Div {
        let shows_identity = super::rows::slack_message_shows_identity(row);
        div()
            .w(px(0.0))
            .min_w(px(0.0))
            .flex_grow(1.0)
            .flex()
            .flex_col()
            .pt(px(SLACK_MESSAGE_EDGE_PADDING))
            .when(shows_identity, |this| {
                this.child(self.render_slack_message_author(
                    row,
                    render_context,
                    selection_context,
                    cx,
                ))
            })
            .when(
                row.body.has_renderable_content() || !row.table_rows.is_empty(),
                |this| {
                    this.child(self.render_slack_message_body(
                        row,
                        render_context,
                        selection_context,
                        cx,
                    ))
                },
            )
            .when(!row.attachments.is_empty(), |this| {
                this.child(
                    self.render_slack_message_attachments(row, render_context, cx)
                        .pt(px(SLACK_MESSAGE_EDGE_PADDING)),
                )
            })
            .when(!row.reactions.is_empty(), |this| {
                this.child(
                    self.render_slack_message_reactions(row, render_context, cx)
                        .pt(px(SLACK_MESSAGE_EDGE_PADDING)),
                )
            })
            .when(row.reply_summary.is_some(), |this| {
                this.child(self.render_slack_message_reply_summary(row, cx))
            })
            .when_some(row.delivery.as_ref(), |this, delivery| {
                this.child(
                    self.render_slack_message_delivery_state(delivery, cx)
                        .pt(px(SLACK_MESSAGE_EDGE_PADDING)),
                )
            })
    }

    fn render_slack_message_body(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        selection_context: Option<&SlackMessageSelectionContext>,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(selection_context) = selection_context else {
            return slack_message_body_block(self, row, render_context, cx);
        };
        slack_message_body_block_in_document(self, row, render_context, selection_context, cx)
    }

    fn render_slack_message_delivery_state(
        &self,
        delivery: &SlackMessageDeliveryState,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        match delivery {
            SlackMessageDeliveryState::Pending { label } => div()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(rgb(palette.main_muted_text))
                .child(label.clone()),
            SlackMessageDeliveryState::Failed {
                label,
                detail,
                actions,
            } => {
                let action_buttons = actions
                    .iter()
                    .map(|action| self.render_slack_message_delivery_action(action, cx))
                    .collect::<Vec<_>>();
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(18.0))
                    .text_color(rgb(0xe01e5a))
                    .child(label.clone())
                    .child(
                        div()
                            .max_w(px(360.0))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_color(rgb(palette.main_muted_text))
                            .child(detail.clone()),
                    )
                    .children(action_buttons)
            }
        }
    }

    fn render_slack_message_delivery_action(
        &self,
        action: &SlackMessageDeliveryAction,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let click_action = action.clone();
        let keyboard_action = action.clone();
        div()
            .id(action.element_id())
            .role(Role::Button)
            .aria_label(action.accessibility_label())
            .focusable()
            .tab_stop(true)
            .px(px(8.0))
            .rounded(px(4.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(palette.link))
            .cursor_pointer()
            .focus_visible(|style| style.bg(rgb(palette.composer_icon_bg)))
            .hover(|style| style.bg(rgb(palette.composer_icon_bg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.activate_slack_outbound_delivery_action(&click_action, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.activate_slack_outbound_delivery_action(&keyboard_action, cx);
            }))
            .child(action.label())
    }
}
