use crate::ui::surface::{
    alpha, slack_activity_palette, slack_base_icon_radius, slack_icon, SlackActivityPalette,
    SlackActivityRow, SlackActivityRowKind, SlackShellIcon, SurfaceState,
};
use gpui::{div, img, point, px, rgb, BoxShadow, Context, Div, FontWeight, ParentElement, Styled};

impl SurfaceState {
    pub(super) fn render_slack_activity_divider(&self, label: gpui::SharedString) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .h(px(44.0))
            .flex_none()
            .mx(px(16.0))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .right(px(0.0))
                    .h(px(1.0))
                    .bg(alpha(palette.card_border, palette.card_border_alpha)),
            )
            .child(
                div()
                    .relative()
                    .h(px(28.0))
                    .px(px(14.0))
                    .rounded_full()
                    .border_1()
                    .border_color(alpha(palette.card_border, palette.card_border_alpha))
                    .bg(rgb(palette.surface_bg))
                    .flex()
                    .items_center()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.primary_text))
                    .child(label),
            )
    }

    pub(super) fn render_slack_activity_avatar(
        &self,
        row: &SlackActivityRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        let avatar = if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            div()
                .size(px(36.0))
                .flex_none()
                .rounded(slack_base_icon_radius(36.0))
                .bg(alpha(palette.primary_text, palette.avatar_backing_alpha))
                .child(img(image).size_full().rounded(slack_base_icon_radius(36.0)))
        } else {
            div()
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
                .child(row.avatar_text.clone())
        };
        avatar
            .relative()
            .child(self.render_slack_activity_kind_badge(row, cx))
    }

    fn render_slack_activity_kind_badge(
        &self,
        row: &SlackActivityRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        let background = if row.unread {
            palette.unread_card_bg
        } else {
            palette.card_bg
        };
        let icon_color = if row.unread {
            palette.primary_text
        } else {
            palette.secondary_text
        };
        let badge = div()
            .absolute()
            .right(px(-5.0))
            .bottom(px(-5.0))
            .size(px(20.0))
            .rounded(px(6.0))
            .bg(rgb(background))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(icon_color));
        match row.kind {
            SlackActivityRowKind::Reaction => {
                badge.child(row.reaction_label.clone().unwrap_or_else(|| "•".into()))
            }
            SlackActivityRowKind::Thread => badge.child(slack_icon(
                SlackShellIcon::MessageFilled,
                icon_color,
                12.0,
                cx,
            )),
            SlackActivityRowKind::Mention => badge.child("@"),
            SlackActivityRowKind::Dm => {
                badge.child(slack_icon(SlackShellIcon::Dm, icon_color, 13.0, cx))
            }
            SlackActivityRowKind::BotDm => {
                badge.child(slack_icon(SlackShellIcon::Apps, icon_color, 12.0, cx))
            }
        }
    }
}

pub(super) fn slack_activity_selected_shadow(palette: SlackActivityPalette) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(palette.selected_ring, 1.0),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(2.0),
        inset: true,
    }]
}

pub(super) fn slack_activity_unread_strip(palette: SlackActivityPalette) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .bottom(px(0.0))
        .w(px(2.0))
        .bg(rgb(palette.importance_bg))
}
