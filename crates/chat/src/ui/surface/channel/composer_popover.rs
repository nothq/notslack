use super::{
    alpha, div, point, px, rgb, slack_icon, AppearanceMode, BoxShadow, Context, Div, FluentBuilder,
    ParentElement, SlackAuxPanelQueryBehavior, SlackAuxPanelState, SlackShellIcon, Styled,
    SurfaceState, SLACK_COMPOSER_ATTACHMENTS_HEIGHT, SLACK_COMPOSER_PICKER_BOTTOM_OFFSET,
    SLACK_COMPOSER_PICKER_LEFT_INSET, SLACK_COMPOSER_PICKER_POPOVER_WIDTH,
    SLACK_EMOJI_PICKER_POPOVER_HEIGHT, SLACK_MENTION_PICKER_POPOVER_HEIGHT,
};

struct SlackComposerSearchFieldColors {
    focused_border: u32,
    idle_border: u32,
    bg: u32,
    text: u32,
    placeholder_text: u32,
    icon: u32,
    shadow_alpha: f32,
    shadow_spread: f32,
}

impl SurfaceState {
    pub(crate) fn render_slack_composer_popover(
        &self,
        panel: &SlackAuxPanelState,
        cx: &mut Context<Self>,
    ) -> Div {
        self.render_slack_composer_popover_with_offsets(
            panel,
            SLACK_COMPOSER_PICKER_LEFT_INSET,
            SLACK_COMPOSER_PICKER_BOTTOM_OFFSET
                + if self.slack_composer_files.is_empty() {
                    0.0
                } else {
                    SLACK_COMPOSER_ATTACHMENTS_HEIGHT
                },
            cx,
        )
    }

    pub(in crate::ui::surface) fn render_slack_composer_popover_with_offsets(
        &self,
        panel: &SlackAuxPanelState,
        left: f32,
        bottom: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let behavior = panel.query_behavior.unwrap_or_default();
        let (width, height) = match behavior {
            SlackAuxPanelQueryBehavior::Emoji => (
                SLACK_COMPOSER_PICKER_POPOVER_WIDTH,
                SLACK_EMOJI_PICKER_POPOVER_HEIGHT,
            ),
            SlackAuxPanelQueryBehavior::Mention | SlackAuxPanelQueryBehavior::SearchWorkspace => (
                SLACK_COMPOSER_PICKER_POPOVER_WIDTH,
                SLACK_MENTION_PICKER_POPOVER_HEIGHT,
            ),
        };
        let (popover_bg, popover_border, shadow_alpha) = match self.appearance_mode {
            AppearanceMode::Dark => (0x1f2226, 0x2c3136, 0.34),
            AppearanceMode::Light => (0xffffff, 0xd0d0d0, 0.18),
        };
        let surface = div()
            .absolute()
            .left(px(left))
            .bottom(px(bottom))
            .w(px(width))
            .h(px(height))
            .rounded(px(10.0))
            .border_1()
            .border_color(rgb(popover_border))
            .bg(rgb(popover_bg))
            .overflow_hidden()
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, shadow_alpha),
                offset: point(px(0.0), px(18.0)),
                blur_radius: px(34.0),
                spread_radius: px(-12.0),
                inset: false,
            }]);
        match behavior {
            SlackAuxPanelQueryBehavior::Emoji => {
                surface.child(self.render_slack_emoji_picker_popover(panel, cx))
            }
            SlackAuxPanelQueryBehavior::Mention => {
                surface.child(self.render_slack_mention_picker_popover(panel, cx))
            }
            SlackAuxPanelQueryBehavior::SearchWorkspace => surface,
        }
    }

    pub(crate) fn render_slack_composer_popover_search_field(
        &self,
        query: &str,
        placeholder: &str,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let is_placeholder = query.is_empty();
        let colors = self.slack_composer_search_field_colors();
        div()
            .h(px(38.0))
            .mx(px(10.0))
            .mt(px(8.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(if focused {
                colors.focused_border
            } else {
                colors.idle_border
            }))
            .bg(rgb(colors.bg))
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(focused, |this| {
                this.shadow(vec![BoxShadow {
                    color: alpha(0x1264a3, colors.shadow_alpha),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(colors.shadow_spread),
                    inset: false,
                }])
            })
            .child(slack_icon(SlackShellIcon::Search, colors.icon, 13.0, cx))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(if is_placeholder {
                        colors.placeholder_text
                    } else {
                        colors.text
                    }))
                    .child(if is_placeholder {
                        placeholder.to_string()
                    } else {
                        query.to_string()
                    }),
            )
    }

    fn slack_composer_search_field_colors(&self) -> SlackComposerSearchFieldColors {
        match self.appearance_mode {
            AppearanceMode::Dark => SlackComposerSearchFieldColors {
                focused_border: 0x1482b7,
                idle_border: 0x3a4046,
                bg: 0x15191d,
                text: 0xf4f5f7,
                placeholder_text: 0x8b9198,
                icon: 0x8b9198,
                shadow_alpha: 0.32,
                shadow_spread: 2.0,
            },
            AppearanceMode::Light => SlackComposerSearchFieldColors {
                focused_border: 0x1264a3,
                idle_border: 0xd0d0d0,
                bg: 0xffffff,
                text: 0x1d1c1d,
                placeholder_text: 0x5e5d60,
                icon: 0x5e5d60,
                shadow_alpha: 0.24,
                shadow_spread: 3.0,
            },
        }
    }
}
