use super::super::{
    div, img, px, rgb, slack_base_icon_radius, slack_palette, AppearanceMode, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, MouseButton, MouseDownEvent, ParentElement,
    SlackPalette, Styled, SurfaceState,
};
use super::SlackDmSidebarMode;
use crate::ui::surface::{SlackDmRow, SlackDmRowParticipant};
use crate::ui::{initials, slack_avatar_fill, SlackConversationKind, SlackUserPresence};

impl SurfaceState {
    pub(super) fn render_slack_dm_inbox_row(
        &self,
        row: &SlackDmRow,
        active_conversation_id: &str,
        mode: SlackDmSidebarMode,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let active = row.conversation_id.as_ref() == active_conversation_id;
        let row_background = slack_dm_row_background(active, mode, self.appearance_mode, palette);
        let rendered = div()
            .w_full()
            .min_h(px(73.0))
            .px(px(16.0))
            .py(px(14.0))
            .border_b_1()
            .border_color(rgb(if mode.is_peek() { 0x515255 } else { 0x424243 }))
            .flex()
            .items_start()
            .gap(px(8.0))
            .bg(rgb(row_background))
            .child(self.render_slack_dm_avatar(row, row_background))
            .child(self.render_slack_dm_text_column(row, active, mode, palette));
        self.bind_slack_dm_row_click(rendered, row, cx)
    }

    fn render_slack_dm_text_column(
        &self,
        row: &SlackDmRow,
        active: bool,
        mode: SlackDmSidebarMode,
        palette: SlackPalette,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .child(slack_dm_title_line(
                row,
                active,
                mode,
                self.appearance_mode,
                palette,
            ))
            .child(slack_dm_preview_line(
                row,
                active,
                mode,
                self.appearance_mode,
                palette,
            ))
    }

    fn render_slack_dm_avatar(&self, row: &SlackDmRow, row_background: u32) -> Div {
        if row.kind == SlackConversationKind::GroupMessage && row.participants.len() >= 2 {
            return self.render_slack_dm_group_avatar(row, row_background);
        }
        let participant = row.participants.first();
        let label = participant
            .map(|participant| participant.label.as_ref())
            .unwrap_or(row.title.as_ref());
        self.render_slack_dm_avatar_participant(participant, label, 36.0, None)
            .relative()
            .when_some(
                participant.and_then(|participant| participant.presence),
                |this, presence| {
                    this.child(slack_dm_presence_badge(
                        presence,
                        self.appearance_mode,
                        row_background,
                    ))
                },
            )
    }

    fn render_slack_dm_group_avatar(&self, row: &SlackDmRow, row_background: u32) -> Div {
        let first = &row.participants[0];
        let second = &row.participants[1];
        div()
            .size(px(36.0))
            .relative()
            .child(
                self.render_slack_dm_avatar_participant(
                    Some(first),
                    first.label.as_ref(),
                    24.0,
                    None,
                )
                .absolute()
                .left(px(0.0))
                .top(px(0.0)),
            )
            .child(
                self.render_slack_dm_avatar_participant(
                    Some(second),
                    second.label.as_ref(),
                    24.0,
                    Some(row_background),
                )
                .absolute()
                .right(px(0.0))
                .bottom(px(0.0)),
            )
    }

    fn render_slack_dm_avatar_participant(
        &self,
        participant: Option<&SlackDmRowParticipant>,
        label: &str,
        size: f32,
        border_color: Option<u32>,
    ) -> Div {
        let avatar = if let Some(image) = participant
            .and_then(|participant| participant.avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            div()
                .size(px(size))
                .rounded(slack_base_icon_radius(size))
                .child(
                    div()
                        .size_full()
                        .rounded(slack_base_icon_radius(size))
                        .overflow_hidden()
                        .child(
                            img(image)
                                .w_full()
                                .h_full()
                                .rounded(slack_base_icon_radius(size)),
                        ),
                )
        } else {
            div()
                .size(px(size))
                .rounded(slack_base_icon_radius(size))
                .bg(rgb(slack_avatar_fill(label)))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(if size < 30.0 { 9.0 } else { 12.0 }))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(initials(label))
        };
        avatar.when_some(border_color, |this, border_color| {
            this.border_2().border_color(rgb(border_color))
        })
    }

    fn bind_slack_dm_row_click(
        &self,
        row_element: Div,
        row: &SlackDmRow,
        cx: &mut Context<Self>,
    ) -> Div {
        if !self.slack_workspace_api_capabilities.load_conversation {
            return row_element;
        }
        let conversation_id = row.conversation_id.to_string();
        row_element.cursor_pointer().on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                this.select_slack_dm_conversation(&conversation_id, cx);
            }),
        )
    }
}

fn slack_dm_title_line(
    row: &SlackDmRow,
    active: bool,
    mode: SlackDmSidebarMode,
    appearance_mode: AppearanceMode,
    palette: SlackPalette,
) -> Div {
    div()
        .min_w(px(0.0))
        .flex()
        .items_start()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex_grow(1.0)
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_normal()
                .line_clamp(2)
                .text_ellipsis()
                .line_height(px(22.0))
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(slack_dm_row_text(
                    active,
                    mode,
                    appearance_mode,
                    palette,
                )))
                .child(row.title.clone()),
        )
        .child(
            div()
                .flex_shrink_0()
                .h(px(22.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .when(!row.timestamp_label.is_empty(), |this| {
                    this.child(
                        div()
                            .line_height(px(22.0))
                            .text_size(px(13.0))
                            .text_color(rgb(slack_dm_timestamp_text(
                                mode,
                                appearance_mode,
                                palette,
                            )))
                            .child(row.timestamp_label.clone()),
                    )
                })
                .when_some(row.mention_count, |this, count| {
                    this.child(slack_dm_count_badge(count))
                }),
        )
}

fn slack_dm_preview_line(
    row: &SlackDmRow,
    active: bool,
    mode: SlackDmSidebarMode,
    appearance_mode: AppearanceMode,
    palette: SlackPalette,
) -> Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_normal()
        .line_clamp(2)
        .text_ellipsis()
        .line_height(px(22.0))
        .text_size(px(14.0))
        .font_weight(FontWeight::NORMAL)
        .text_color(rgb(slack_dm_preview_text(
            active,
            mode,
            appearance_mode,
            palette,
        )))
        .child(row.preview.clone())
}

fn slack_dm_count_badge(count: u32) -> Div {
    div()
        .min_w(px(20.0))
        .h(px(20.0))
        .px(px(6.0))
        .rounded_full()
        .bg(rgb(0xdd4e76))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child(count.to_string())
}

fn slack_dm_row_background(
    active: bool,
    mode: SlackDmSidebarMode,
    appearance_mode: AppearanceMode,
    palette: SlackPalette,
) -> u32 {
    if mode.is_peek() {
        mode.background(palette.sidebar_bg)
    } else if active && appearance_mode == AppearanceMode::Dark {
        0x212223
    } else if active {
        palette.sidebar_active_bg
    } else {
        palette.sidebar_bg
    }
}

fn slack_dm_row_text(
    active: bool,
    _mode: SlackDmSidebarMode,
    appearance_mode: AppearanceMode,
    palette: SlackPalette,
) -> u32 {
    if appearance_mode == AppearanceMode::Dark {
        0xf8f8f8
    } else if active {
        palette.sidebar_active_text
    } else {
        palette.sidebar_text
    }
}

fn slack_dm_preview_text(
    active: bool,
    mode: SlackDmSidebarMode,
    appearance_mode: AppearanceMode,
    palette: SlackPalette,
) -> u32 {
    if appearance_mode == AppearanceMode::Dark {
        if mode.is_peek() {
            0xb9babd
        } else {
            0xf8f8f8
        }
    } else if active {
        palette.sidebar_active_text
    } else {
        palette.sidebar_text
    }
}

fn slack_dm_timestamp_text(
    mode: SlackDmSidebarMode,
    appearance_mode: AppearanceMode,
    palette: SlackPalette,
) -> u32 {
    if appearance_mode == AppearanceMode::Dark {
        if mode.is_peek() {
            0xbebebf
        } else {
            0xbababa
        }
    } else {
        palette.sidebar_muted_text
    }
}

fn slack_dm_presence_badge(
    presence: SlackUserPresence,
    appearance_mode: AppearanceMode,
    row_background: u32,
) -> Div {
    let palette = slack_palette(appearance_mode);
    let (fill, border) = match presence {
        SlackUserPresence::Active => (0x2bac76, row_background),
        SlackUserPresence::Away => (row_background, palette.sidebar_icon),
    };
    div()
        .absolute()
        .right(px(-2.0))
        .bottom(px(-2.0))
        .size(px(9.0))
        .rounded_full()
        .bg(rgb(fill))
        .border_1()
        .border_color(rgb(border))
}
