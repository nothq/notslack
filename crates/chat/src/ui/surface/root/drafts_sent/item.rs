use super::super::super::{
    alpha, div, img, px, rgb, slack_base_icon_radius, slack_icon, AnyElement, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SlackDraftsSentFileCard, SlackDraftsSentFilePresentation, SlackDraftsSentItemRow,
    SlackDraftsSentRowTarget, SlackScheduledEdit, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use super::drafts_sent_action_key;
use crate::ui::SlackDraftsSentTab;
use gpui::{ObjectFit, Role, Stateful, StyledImage};

impl SurfaceState {
    pub(super) fn render_slack_drafts_sent_item(
        &self,
        row: &SlackDraftsSentItemRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let click_target = row.target.clone();
        let keyboard_target = row.target.clone();
        let sent_card = self.slack_drafts_sent_tab == SlackDraftsSentTab::Sent;
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_position_in_set(index + 1)
            .aria_size_of_set(self.slack_drafts_sent_rows.len())
            .focusable()
            .tab_stop(true)
            .h(px(row.height()))
            .when(!sent_card, |this| this.w_full())
            .when(sent_card, |this| {
                this.mx(px(20.0))
                    .border_l_1()
                    .border_r_1()
                    .border_color(alpha(0x000000, 0.1))
                    .bg(rgb(0x1a1d21))
            })
            .when(sent_card && row.sent_group_first, |this| {
                this.border_t_1().rounded_t(px(12.0))
            })
            .when(sent_card && !row.sent_group_first, |this| this.border_t_1())
            .when(sent_card && row.sent_group_last, |this| {
                this.border_b_1().rounded_b(px(12.0))
            })
            .px(px(16.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x222529)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_drafts_sent_target(click_target.clone(), cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if drafts_sent_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_drafts_sent_target(keyboard_target.clone(), cx);
                }
            }))
            .flex()
            .items_start()
            .gap(px(12.0))
            .child(self.render_slack_drafts_sent_avatar(row))
            .child(self.render_slack_drafts_sent_item_body(row, cx))
            .when(
                matches!(&row.target, SlackDraftsSentRowTarget::Scheduled { .. }),
                |this| this.child(self.render_slack_scheduled_item_actions(row, cx)),
            )
            .child(drafts_sent_item_timestamp(row))
            .into_any_element()
    }

    fn render_slack_drafts_sent_item_body(
        &self,
        row: &SlackDraftsSentItemRow,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .min_w(px(0.0))
            .flex_grow(1.0)
            .pt(px(12.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(22.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(15.0))
                    .line_height(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child(row.destination.clone()),
            )
            .child(
                div()
                    .h(px(22.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(15.0))
                    .line_height(px(22.0))
                    .text_color(alpha(0xe8e8e8, 0.7))
                    .child(row.body.clone()),
            )
            .when(row.files.has_files(), |this| {
                this.child(self.render_slack_drafts_sent_file_cards(row, cx))
            })
    }

    fn render_slack_drafts_sent_file_cards(
        &self,
        row: &SlackDraftsSentItemRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let cards = match &row.files {
            SlackDraftsSentFilePresentation::None => Vec::new(),
            SlackDraftsSentFilePresentation::Pending { references }
            | SlackDraftsSentFilePresentation::Loading { references } => references
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    self.render_slack_drafts_sent_file_card(
                        row,
                        index,
                        SlackDraftsSentFileCard::Loading,
                        cx,
                    )
                })
                .collect(),
            SlackDraftsSentFilePresentation::Loaded { cards, .. } => cards
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, card)| self.render_slack_drafts_sent_file_card(row, index, card, cx))
                .collect(),
        };
        div()
            .mt(px(12.0))
            .h(px(40.0))
            .min_w(px(0.0))
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(8.0))
            .children(cards)
    }

    fn render_slack_drafts_sent_file_card(
        &self,
        row: &SlackDraftsSentItemRow,
        index: usize,
        card: SlackDraftsSentFileCard,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (label, image, icon) = match card {
            SlackDraftsSentFileCard::Loading => {
                ("Loading file…".to_string(), None, SlackShellIcon::Files)
            }
            SlackDraftsSentFileCard::Loaded(attachment) => (
                attachment.title.clone(),
                attachment
                    .preview_image_url
                    .as_deref()
                    .and_then(|url| self.slack_remote_images.get(url).cloned()),
                SlackShellIcon::Files,
            ),
            SlackDraftsSentFileCard::Unavailable => (
                "File unavailable".to_string(),
                None,
                SlackShellIcon::WarningFilled,
            ),
        };
        let has_image = image.is_some();
        div()
            .id(format!("slack-drafts-sent-file-{}-{index}", row.id))
            .role(Role::Group)
            .aria_label(label)
            .size(px(40.0))
            .flex_none()
            .rounded(px(4.0))
            .border_1()
            .border_color(alpha(0xd1d2d3, 0.18))
            .bg(rgb(0x2c2f33))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .when_some(image, |this, image| {
                this.child(img(image).size_full().object_fit(ObjectFit::Cover))
            })
            .when(!has_image, |this| {
                this.child(slack_icon(icon, 0xb9babd, 20.0, cx))
            })
            .into_any_element()
    }

    fn render_slack_scheduled_item_actions(
        &self,
        row: &SlackDraftsSentItemRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackDraftsSentRowTarget::Scheduled { edit, document } = &row.target else {
            return div().into_any_element();
        };
        let deleting = self
            .slack_scheduled_delete_pending
            .as_ref()
            .is_some_and(|pending| pending.target == edit.target);
        let mutation_pending = self.slack_drafts_sent_draft_activation_is_blocked();
        let edit_target = SlackDraftsSentRowTarget::Scheduled {
            edit: edit.clone(),
            document: document.clone(),
        };
        div()
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(self.render_slack_scheduled_edit_action(row, edit_target, mutation_pending, cx))
            .child(self.render_slack_scheduled_cancel_action(row, edit.clone(), deleting, cx))
            .into_any_element()
    }

    fn render_slack_scheduled_edit_action(
        &self,
        row: &SlackDraftsSentItemRow,
        target: SlackDraftsSentRowTarget,
        mutation_pending: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let keyboard_target = target.clone();
        div()
            .id(format!("slack-scheduled-edit-{}", row.id))
            .role(Role::Button)
            .aria_label("Edit scheduled message")
            .h(px(28.0))
            .px(px(8.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(alpha(0x797c81, 0.55))
            .text_size(px(12.0))
            .text_color(rgb(if mutation_pending { 0x797c81 } else { 0xd1d2d3 }))
            .flex()
            .items_center()
            .justify_center()
            .when(!mutation_pending, |this| {
                this.focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x2c2f33)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.activate_slack_drafts_sent_target(target.clone(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event, window, cx| {
                        if drafts_sent_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.activate_slack_drafts_sent_target(keyboard_target.clone(), cx);
                        }
                    }))
            })
            .child("Edit")
    }

    fn render_slack_scheduled_cancel_action(
        &self,
        row: &SlackDraftsSentItemRow,
        target: SlackScheduledEdit,
        deleting: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let keyboard_target = target.clone();
        div()
            .id(format!("slack-scheduled-cancel-{}", row.id))
            .role(Role::Button)
            .aria_label("Cancel scheduled message")
            .h(px(28.0))
            .px(px(8.0))
            .rounded(px(4.0))
            .text_size(px(12.0))
            .text_color(rgb(if deleting { 0x797c81 } else { 0xe01e5a }))
            .flex()
            .items_center()
            .justify_center()
            .when(!deleting, |this| {
                this.focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(0xe01e5a, 0.12)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.delete_slack_scheduled_draft(target.clone(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event, window, cx| {
                        if drafts_sent_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.delete_slack_scheduled_draft(keyboard_target.clone(), cx);
                        }
                    }))
            })
            .child(if deleting { "Canceling…" } else { "Cancel" })
    }

    fn render_slack_drafts_sent_avatar(&self, row: &SlackDraftsSentItemRow) -> Div {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .mt(px(16.0))
                .size(px(36.0))
                .flex_none()
                .rounded(slack_base_icon_radius(36.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(36.0)));
        }
        div()
            .mt(px(16.0))
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
            .child(row.avatar_label.clone())
    }
}

fn drafts_sent_item_timestamp(row: &SlackDraftsSentItemRow) -> Div {
    div()
        .flex_none()
        .pt(px(12.0))
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(alpha(0xe8e8e8, 0.7))
        .child(row.timestamp.clone())
}
