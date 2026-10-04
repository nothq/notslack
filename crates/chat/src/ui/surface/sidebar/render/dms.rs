use super::{
    div, list, px, rgb, slack_palette, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ListSizingBehavior, MouseButton, MouseDownEvent,
    ParentElement, Styled, SurfaceState,
};
use crate::ui::SlackWorkspace;

mod finder;
mod row;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackDmSidebarMode {
    Full,
    Peek,
}

impl SlackDmSidebarMode {
    pub(crate) fn is_peek(self) -> bool {
        self == Self::Peek
    }

    pub(crate) fn background(self, default: u32) -> u32 {
        if self.is_peek() {
            0x222428
        } else {
            default
        }
    }

    fn header_height(self) -> f32 {
        if self.is_peek() {
            46.0
        } else {
            49.0
        }
    }

    fn header_text(self, default: u32) -> u32 {
        if self.is_peek() {
            0xd1d2d3
        } else {
            default
        }
    }

    fn title_size(self) -> f32 {
        if self.is_peek() {
            16.0
        } else {
            20.0
        }
    }

    fn control(self) -> u32 {
        if self.is_peek() {
            0xb9babd
        } else {
            0xbababa
        }
    }
}

impl SurfaceState {
    pub(crate) fn render_slack_dm_sidebar(
        &self,
        workspace: &SlackWorkspace,
        mode: SlackDmSidebarMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let finder_active = !mode.is_peek() && self.slack_dm_finder_active();
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(
                mode.background(slack_palette(self.appearance_mode).sidebar_bg)
            ))
            .when(finder_active, |this| {
                this.child(self.render_slack_sidebar_header(workspace, cx).h(px(45.0)))
                    .child(self.render_slack_dm_finder_search(true, cx))
                    .child(self.render_slack_dm_finder_results(cx))
            })
            .when(!finder_active, |this| {
                this.child(self.render_slack_dm_sidebar_header(mode, cx))
                    .when(!mode.is_peek(), |this| {
                        this.child(self.render_slack_dm_finder_search(false, cx))
                    })
                    .child(self.render_slack_dm_sidebar_list(workspace, mode, cx))
            })
            .into_any_element()
    }

    fn render_slack_dm_sidebar_header(
        &self,
        mode: SlackDmSidebarMode,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(mode.header_height()))
            .flex_shrink_0()
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .h(px(32.0))
                    .px(px(8.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(mode.title_size()))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(mode.header_text(palette.sidebar_header_text)))
                            .child("Direct messages"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_slack_dm_unread_toggle(mode, cx)),
            )
    }

    fn render_slack_dm_unread_toggle(
        &self,
        mode: SlackDmSidebarMode,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let active = self.slack_dms_show_unread_only;
        let control = mode.control();
        let background = mode.background(palette.sidebar_bg);
        div()
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(6.0))
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(control))
            .child("Unreads")
            .child(
                div()
                    .w(px(28.0))
                    .h(px(18.0))
                    .rounded(px(9.0))
                    .bg(rgb(if active {
                        palette.composer_focused_border
                    } else {
                        background
                    }))
                    .border_1()
                    .border_color(rgb(if active {
                        palette.composer_focused_border
                    } else {
                        control
                    }))
                    .flex()
                    .items_center()
                    .justify_end()
                    .p(px(1.0))
                    .when(!active, |this| this.justify_start())
                    .child(div().size(px(14.0)).rounded_full().bg(rgb(control))),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.toggle_slack_dms_unread_filter(cx);
                }),
            )
    }

    fn render_slack_dm_sidebar_list(
        &self,
        workspace: &SlackWorkspace,
        mode: SlackDmSidebarMode,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self.slack_dm_rows.clone();
        let visible_row_indices = self.slack_dm_visible_row_indices.clone();
        let active_conversation_id = self
            .slack_pending_conversation_id
            .clone()
            .unwrap_or_else(|| workspace.conversation_id.clone());
        let view = cx.entity();
        let dm_list = list(
            self.slack_dm_list_state.clone(),
            move |index, _window, cx| {
                let rows = rows.clone();
                let visible_row_indices = visible_row_indices.clone();
                let active_conversation_id = active_conversation_id.clone();
                view.update(cx, move |this, cx| {
                    let row_index = *visible_row_indices
                        .get(index)
                        .expect("visible Slack DM inbox row index should exist");
                    let row = rows
                        .get(row_index)
                        .expect("prepared Slack DM inbox row should exist");
                    this.render_slack_dm_inbox_row(row, &active_conversation_id, mode, cx)
                        .into_any_element()
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full();
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .child(dm_list)
            .into_any_element()
    }
}
