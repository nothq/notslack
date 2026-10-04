use super::super::super::{
    alpha, div, point, px, relative, rgb, AnyElement, BoxShadow, Context, Div, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Styled, SurfaceState,
    SLACK_TOP_NAV_LEFT_FLEX_BASIS, SLACK_TOP_NAV_LEFT_INSET, SLACK_TOP_NAV_RIGHT_INSET,
    SLACK_TOP_SEARCH_MAX_WIDTH, SLACK_TOP_SEARCH_MIN_WIDTH,
};
use super::slack_search_overlay_palette;
use crate::ui::surface::SlackSearchUnreadDraftPosition;
use crate::ui::AppearanceMode;
use crate::ui::SLACK_TOP_NAV_RAIL_WIDTH;
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

mod actions;
mod header;
mod messages;
mod quick;
mod recent;

const SLACK_SEARCH_HEADER_TOP: f32 = 4.0;
pub(super) const SLACK_SEARCH_HEADER_HEIGHT: f32 = 49.0;
pub(super) const SLACK_SEARCH_ACTION_HEIGHT: f32 = 40.0;
pub(super) const SLACK_SEARCH_RECENT_HEADING_HEIGHT: f32 = 36.0;
pub(super) const SLACK_SEARCH_RECENT_ROW_HEIGHT: f32 = 40.0;
pub(super) const SLACK_QUICK_SEARCH_ROW_HEIGHT: f32 = 40.0;
pub(super) const SLACK_QUICK_SEARCH_MAX_VISIBLE_ROWS: usize = 6;
pub(super) const SLACK_QUICK_SEARCH_MESSAGE_BOTTOM_SPACE: f32 = 10.0;
pub(super) const SLACK_QUICK_SEARCH_MESSAGE_DIVIDER_HEIGHT: f32 = 16.0;
pub(super) const SLACK_QUICK_SEARCH_MESSAGE_HEADING_HEIGHT: f32 = 36.0;
pub(super) const SLACK_QUICK_SEARCH_MESSAGE_ROW_HEIGHT: f32 = 78.0;
const SLACK_SEARCH_EMPTY_HEIGHT: f32 = 48.0;
const SLACK_SEARCH_DIALOG_LEFT_EXPANSION: f32 = 7.679_7;
const SLACK_SEARCH_DIALOG_RIGHT_EXPANSION: f32 = 8.437_5;
pub(super) const SLACK_SEARCH_DIALOG_RIGHT_LANE_WIDTH: f32 = 78.0;
pub(super) const SLACK_SEARCH_FOOTER_HEIGHT: f32 = 45.0;

/// A search overlay row's place among the listbox options.
#[derive(Clone, Copy)]
struct SlackSearchOptionPosition {
    option_index: usize,
    option_count: usize,
    selected: bool,
}

struct SlackSearchDialogContext {
    unread_drafts: Vec<SlackSearchUnreadDraftPosition>,
    recent_history_indices: Vec<usize>,
    option_count: usize,
    height: f32,
}

impl SurfaceState {
    pub(in super::super) fn render_slack_search_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = slack_search_dialog_context(self);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, window, cx| {
                this.close_slack_search(cx);
                cx.focus_self(window);
            }),
            cx,
        );
        div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .occlude()
            .pl(px(SLACK_TOP_NAV_RAIL_WIDTH + SLACK_TOP_NAV_LEFT_INSET))
            .pr(px(SLACK_TOP_NAV_RIGHT_INSET))
            .flex()
            .child(backdrop)
            .child(div().w(relative(SLACK_TOP_NAV_LEFT_FLEX_BASIS)).flex_none())
            .child(self.render_slack_search_dialog(dialog, cx))
            .child(slack_search_right_lane())
            .into_any_element()
    }

    fn render_slack_search_dialog(
        &self,
        dialog: SlackSearchDialogContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_grow(2.0)
            .flex_shrink(1.0)
            .flex_basis(relative(0.0))
            .min_w(px(SLACK_TOP_SEARCH_MIN_WIDTH))
            .max_w(px(SLACK_TOP_SEARCH_MAX_WIDTH))
            .h_full()
            .relative()
            .child(
                self.slack_search_overlay_shell(dialog.height)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .flex()
                    .flex_col()
                    .child(self.render_slack_search_dialog_header(cx))
                    .child(self.render_slack_search_dialog_content(
                        &dialog.unread_drafts,
                        &dialog.recent_history_indices,
                        cx,
                    ))
                    .child(self.render_slack_search_footer(dialog.option_count > 0, cx)),
            )
    }

    fn slack_search_overlay_shell(&self, height: f32) -> Div {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .absolute()
            .top(px(SLACK_SEARCH_HEADER_TOP))
            .left(px(-SLACK_SEARCH_DIALOG_LEFT_EXPANSION))
            .right(px(-SLACK_SEARCH_DIALOG_RIGHT_EXPANSION))
            .h(px(height))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.surface))
            .shadow(slack_search_overlay_shadow(self.appearance_mode))
            .overflow_hidden()
    }
}

fn slack_search_dialog_context(state: &SurfaceState) -> SlackSearchDialogContext {
    let has_api_query = state.slack_search_has_api_query();
    let recent_history_indices = if has_api_query {
        Vec::new()
    } else {
        state.slack_search_recent_history_indices()
    };
    let unread_drafts = if has_api_query {
        Vec::new()
    } else {
        state.slack_search_unreads_and_drafts()
    };
    let option_count = if has_api_query {
        state.slack_search_option_count()
    } else {
        unread_drafts.len() + recent_history_indices.len()
    };
    let content_height = if has_api_query {
        quick::slack_quick_search_content_height(state)
            + SLACK_SEARCH_ACTION_HEIGHT
                * (1 + usize::from(state.slack_search_has_current_conversation_action())) as f32
            + usize::from(!state.slack_quick_search_message_rows.is_empty()) as f32
                * (SLACK_QUICK_SEARCH_MESSAGE_DIVIDER_HEIGHT
                    + SLACK_QUICK_SEARCH_MESSAGE_HEADING_HEIGHT)
            + SLACK_QUICK_SEARCH_MESSAGE_ROW_HEIGHT
                * state.slack_quick_search_message_rows.len() as f32
            + usize::from(!state.slack_quick_search_message_rows.is_empty()) as f32
                * SLACK_QUICK_SEARCH_MESSAGE_BOTTOM_SPACE
    } else if unread_drafts.is_empty() && recent_history_indices.is_empty() {
        SLACK_SEARCH_EMPTY_HEIGHT
    } else {
        usize::from(!unread_drafts.is_empty()) as f32 * SLACK_SEARCH_RECENT_HEADING_HEIGHT
            + SLACK_SEARCH_RECENT_ROW_HEIGHT * unread_drafts.len() as f32
            + usize::from(!recent_history_indices.is_empty()) as f32
                * SLACK_SEARCH_RECENT_HEADING_HEIGHT
            + SLACK_SEARCH_RECENT_ROW_HEIGHT * recent_history_indices.len() as f32
    };
    SlackSearchDialogContext {
        unread_drafts,
        recent_history_indices,
        option_count,
        height: (SLACK_SEARCH_HEADER_HEIGHT + content_height + SLACK_SEARCH_FOOTER_HEIGHT).min(
            (state.viewport_height - SLACK_SEARCH_HEADER_TOP - 8.0)
                .max(SLACK_SEARCH_HEADER_HEIGHT + SLACK_SEARCH_FOOTER_HEIGHT),
        ),
    }
}

fn slack_search_right_lane() -> Div {
    div()
        .flex_auto()
        .flex_shrink_0()
        .w(px(SLACK_SEARCH_DIALOG_RIGHT_LANE_WIDTH))
}

fn slack_search_overlay_shadow(appearance_mode: AppearanceMode) -> Vec<BoxShadow> {
    match appearance_mode {
        AppearanceMode::Light => vec![
            BoxShadow {
                color: alpha(0x1d1c1d, 0.13),
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
                inset: false,
            },
            BoxShadow {
                color: alpha(0x000000, 0.16),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(12.0),
                spread_radius: px(0.0),
                inset: false,
            },
        ],
        AppearanceMode::Dark => vec![BoxShadow {
            color: alpha(0x000000, 0.42),
            offset: point(px(0.0), px(8.0)),
            blur_radius: px(28.0),
            spread_radius: px(-8.0),
            inset: false,
        }],
    }
}
