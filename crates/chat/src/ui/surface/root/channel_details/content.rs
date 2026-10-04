use super::super::super::{
    div, px, rgb, slack_palette, FluentBuilder, FontWeight, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::PreparedSlackChannelDetails;
use gpui::{Div, SharedString};

impl SurfaceState {
    pub(super) fn render_slack_channel_details_body(&self) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-channel-details-body")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_y_scroll()
            .px(px(28.0))
            .pt(px(16.0))
            .pb(px(28.0))
            .text_color(rgb(palette.main_text))
            .when_some(self.slack_channel_details.as_ref(), |this, details| {
                this.child(self.render_slack_channel_details_loaded(details))
            })
            .when(
                self.slack_channel_details.is_none() && self.slack_channel_details_loading,
                |this| {
                    this.child(self.render_slack_channel_details_status("Loading channel details…"))
                },
            )
            .when_some(self.slack_channel_details_error.as_ref(), |this, error| {
                this.child(self.render_slack_channel_details_status(error.clone()))
            })
    }

    fn render_slack_channel_details_loaded(&self, details: &PreparedSlackChannelDetails) -> Div {
        let has_secondary_rows =
            details.topic.is_some() || details.description.is_some() || details.creation.is_some();
        div()
            .w_full()
            .child(self.render_slack_channel_details_name_card(details))
            .when(has_secondary_rows, |this| {
                this.child(self.render_slack_channel_details_secondary_card(details))
            })
    }

    fn render_slack_channel_details_name_card(&self, details: &PreparedSlackChannelDetails) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w_full()
            .rounded(px(12.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .overflow_hidden()
            .bg(rgb(palette.main_bg))
            .child(self.render_slack_channel_details_row(
                "Channel name",
                details.name.clone(),
                true,
            ))
    }

    fn render_slack_channel_details_secondary_card(
        &self,
        details: &PreparedSlackChannelDetails,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .mt(px(16.0))
            .w_full()
            .rounded(px(12.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .overflow_hidden()
            .bg(rgb(palette.main_bg))
            .when_some(details.topic.as_ref(), |card, topic| {
                card.child(self.render_slack_channel_details_row(
                    "Topic",
                    topic.clone(),
                    details.description.is_none() && details.creation.is_none(),
                ))
            })
            .when_some(details.description.as_ref(), |card, description| {
                card.child(self.render_slack_channel_details_row(
                    "Description",
                    description.clone(),
                    details.creation.is_none(),
                ))
            })
            .when_some(details.creation.as_ref(), |card, creation| {
                card.child(self.render_slack_channel_details_row(
                    creation.row_label,
                    creation.value.clone(),
                    true,
                ))
            })
    }

    fn render_slack_channel_details_status(&self, message: impl Into<SharedString>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w_full()
            .rounded(px(12.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .bg(rgb(palette.main_bg))
            .px(px(20.0))
            .py(px(18.0))
            .text_size(px(15.0))
            .line_height(px(22.0))
            .text_color(rgb(palette.main_secondary_text))
            .child(message.into())
    }

    fn render_slack_channel_details_row(
        &self,
        label: &'static str,
        value: SharedString,
        last: bool,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .min_h(px(76.0))
            .px(px(20.0))
            .py(px(14.0))
            .when(!last, |this| {
                this.border_b_1().border_color(rgb(palette.main_border))
            })
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(3.0))
            .child(channel_details_row_label(label, palette.main_text))
            .child(channel_details_row_value(value, palette.main_text))
    }
}

fn channel_details_row_label(label: &'static str, text_color: u32) -> Div {
    div()
        .text_size(px(15.0))
        .line_height(px(22.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(text_color))
        .child(label)
}

fn channel_details_row_value(value: SharedString, text_color: u32) -> Div {
    div()
        .text_size(px(15.0))
        .line_height(px(22.0))
        .text_color(rgb(text_color))
        .child(value)
}
