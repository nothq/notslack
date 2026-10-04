use crate::ui::surface::state::SlackDateJumpMenuTarget;
use crate::ui::surface::{
    alpha, div, point, px, relative, rgb, slack_icon, slack_palette, AppearanceMode, BoxShadow,
    Context, Div, FluentBuilder, FontWeight, InteractiveElement, KeyDownEvent, ParentElement,
    SlackDateJumpOverlay, SlackMessageDividerRow, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use gpui::{ClickEvent, Hsla, Role, Stateful};

struct SlackDateDividerStyle {
    background: Hsla,
    hover_background: Hsla,
    text: u32,
    caret: u32,
    radius: f32,
    shadow: Option<Vec<BoxShadow>>,
}

impl SurfaceState {
    pub(super) fn render_slack_message_date_divider(
        &self,
        divider: &SlackMessageDividerRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let style = slack_date_divider_style(self.appearance_mode);
        let interactive = divider.local_date.is_some()
            && self
                .slack_workspace_api_capabilities
                .navigate_conversation_dates;
        let menu_open = self.slack_date_divider_menu_open(divider);
        let pill = self.render_slack_message_date_pill(divider, style, interactive, cx);
        slack_message_date_divider_shell(divider, pill, menu_open, palette.main_border, cx)
    }

    fn slack_date_divider_menu_open(&self, divider: &SlackMessageDividerRow) -> bool {
        self.slack_date_jump_overlay
            .as_ref()
            .is_some_and(|overlay| {
                matches!(
                    overlay,
                    SlackDateJumpOverlay::Menu(menu) if menu.divider_id == divider.element_id
                )
            })
    }

    fn render_slack_message_date_pill(
        &self,
        divider: &SlackMessageDividerRow,
        style: SlackDateDividerStyle,
        interactive: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let hover_background = style.hover_background;
        let pill = div()
            .id(divider.element_id.clone())
            .relative()
            .h(px(28.0))
            .pl(px(16.0))
            .pr(px(if interactive { 8.0 } else { 16.0 }))
            .rounded(px(style.radius))
            .bg(style.background)
            .when_some(style.shadow, |this, shadow| this.shadow(shadow))
            .flex()
            .items_center()
            .text_size(px(14.0))
            .line_height(px(27.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(style.text))
            .child(divider.label.clone())
            .when(interactive, |this| {
                this.child(
                    div()
                        .ml(px(4.0))
                        .size(px(13.0))
                        .flex_none()
                        .child(slack_icon(
                            SlackShellIcon::DateDividerChevron,
                            style.caret,
                            13.0,
                            cx,
                        )),
                )
            });
        if interactive {
            self.bind_slack_date_divider_pill(pill, divider, hover_background, cx)
        } else {
            pill
        }
    }

    fn bind_slack_date_divider_pill(
        &self,
        pill: Stateful<Div>,
        divider: &SlackMessageDividerRow,
        hover_background: Hsla,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let click_divider_id = divider.element_id.clone();
        let click_local_date = divider.local_date;
        let keyboard_divider_id = divider.element_id.clone();
        let keyboard_local_date = divider.local_date;
        pill.role(Role::Button)
            .aria_label(divider.accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .hover(move |style| style.bg(hover_background))
            .focus_visible(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                let Some(local_date) = click_local_date else {
                    return;
                };
                let position = event.position();
                this.open_slack_date_jump_menu(
                    SlackDateJumpMenuTarget {
                        divider_id: click_divider_id.clone(),
                        source_date: local_date,
                        anchor_x: position.x.as_f32(),
                        anchor_y: position.y.as_f32(),
                    },
                    cx,
                );
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                let Some(local_date) = keyboard_local_date else {
                    return;
                };
                window.prevent_default();
                cx.stop_propagation();
                this.open_slack_date_jump_menu(
                    SlackDateJumpMenuTarget {
                        divider_id: keyboard_divider_id.clone(),
                        source_date: local_date,
                        anchor_x: this.preview_width / 2.0,
                        anchor_y: this.viewport_height / 2.0,
                    },
                    cx,
                );
            }))
    }

    pub(in crate::ui::surface) fn render_slack_message_unread_divider(&self) -> Div {
        div().w_full().h(px(0.0)).relative().child(
            div()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .top(px(0.0))
                .h(px(0.0))
                .flex()
                .items_start()
                .child(
                    div()
                        .h(px(1.0))
                        .mb(px(-1.0))
                        .flex_grow(1.0)
                        .bg(rgb(0xe01e5a))
                        .opacity(0.5),
                )
                .child(
                    div()
                        .h(px(19.0))
                        .mt(px(-10.0))
                        .mr(px(15.0))
                        .px(px(4.0))
                        .text_size(px(13.0))
                        .line_height(relative(1.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xe01e5a))
                        .child("New"),
                ),
        )
    }
}

fn slack_message_date_divider_shell(
    divider: &SlackMessageDividerRow,
    pill: Stateful<Div>,
    menu_open: bool,
    border_color: u32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let anchor_divider_id = divider.element_id.clone();
    let surface = cx.entity().downgrade();
    div()
        .w_full()
        .pb(px(18.0))
        .relative()
        .flex()
        .justify_center()
        .child(
            div()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .top(px(14.0))
                .h(px(1.0))
                .bg(rgb(border_color)),
        )
        .child(
            div()
                .w_full()
                .flex()
                .justify_center()
                .when(menu_open, |this| {
                    this.on_children_prepainted(move |bounds, window, cx| {
                        let Some(bounds) = bounds.first().copied() else {
                            return;
                        };
                        let should_refresh = surface
                            .update(cx, |this, _cx| {
                                this.update_slack_date_jump_anchor(
                                    &anchor_divider_id,
                                    bounds.left().as_f32(),
                                    bounds.bottom().as_f32() + 5.0,
                                )
                            })
                            .unwrap_or(false);
                        if should_refresh {
                            window.refresh();
                        }
                    })
                })
                .child(pill),
        )
}

fn slack_date_divider_style(appearance_mode: AppearanceMode) -> SlackDateDividerStyle {
    match appearance_mode {
        AppearanceMode::Dark => SlackDateDividerStyle {
            background: alpha(0x1a1d21, 1.0),
            hover_background: alpha(0x222529, 1.0),
            text: 0xd1d2d3,
            caret: 0xd1d2d3,
            radius: 9999.0,
            shadow: None,
        },
        AppearanceMode::Light => SlackDateDividerStyle {
            background: alpha(0xffffff, 1.0),
            hover_background: alpha(0xf8f8f8, 1.0),
            text: 0x1d1c1d,
            caret: 0x1d1c1d,
            radius: 24.0,
            shadow: Some(vec![
                BoxShadow {
                    color: alpha(0x1d1c1d, 0.13),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(1.0),
                    inset: false,
                },
                BoxShadow {
                    color: alpha(0x000000, 0.08),
                    offset: point(px(0.0), px(1.0)),
                    blur_radius: px(3.0),
                    spread_radius: px(0.0),
                    inset: false,
                },
            ]),
        },
    }
}
