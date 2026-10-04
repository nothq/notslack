use super::dms::SlackDmSidebarMode;
use super::{
    alpha, build_slack_sidebar_rows, div, list, point, px, rgb, slack_icon, slack_palette,
    AnyElement, BoxShadow, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    IntoElement, ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement, SlackPalette,
    SlackRailView, SlackShellIcon, SlackSidebarRow, Styled, SurfaceState,
    SLACK_SIDEBAR_HEADER_HEIGHT,
};
use crate::ui::{
    SlackWorkspace, SLACK_MAIN_MIN_WIDTH, SLACK_SIDEBAR_MAX_WIDTH, SLACK_SIDEBAR_MIN_WIDTH,
    SLACK_WORKSPACE_FRAME_RIGHT_MARGIN,
};
use gpui::{KeyDownEvent, ListOffset, Role, Stateful, StatefulInteractiveElement};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

const SLACK_DM_PEEK_TOP: f32 = 83.0;
const SLACK_DM_PEEK_WIDTH: f32 = 362.0;
const SLACK_DM_PEEK_MAX_HEIGHT: f32 = 689.0;

mod scrolling;
mod upgrade;

use scrolling::slack_sidebar_boundary_targets;

impl SurfaceState {
    pub(crate) fn slack_sidebar_available_width(&self) -> f32 {
        (self.preview_width - self.slack_rail_width() - SLACK_WORKSPACE_FRAME_RIGHT_MARGIN).max(0.0)
    }

    pub(crate) fn slack_sidebar_width_bounds(&self) -> (f32, f32) {
        let workspace_width = self.slack_sidebar_available_width();
        let max_width = (workspace_width - SLACK_MAIN_MIN_WIDTH)
            .clamp(SLACK_SIDEBAR_MIN_WIDTH, SLACK_SIDEBAR_MAX_WIDTH)
            .min(workspace_width);
        let min_width = SLACK_SIDEBAR_MIN_WIDTH.min(max_width);
        (min_width, max_width)
    }

    pub(crate) fn slack_sidebar_width(&self) -> f32 {
        let (min_width, max_width) = self.slack_sidebar_width_bounds();
        self.legacy_slack_sidebar_width
            .unwrap_or(self.slack_sidebar_available_width() * self.slack_sidebar_preferred_ratio)
            .round()
            .clamp(min_width, max_width)
    }

    pub(crate) fn set_slack_sidebar_preferred_ratio(
        &mut self,
        ratio: f32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.legacy_slack_sidebar_width.is_none()
            && (self.slack_sidebar_preferred_ratio - ratio).abs() < f32::EPSILON
        {
            return false;
        }
        self.slack_sidebar_preferred_ratio = ratio;
        self.legacy_slack_sidebar_width = None;
        cx.notify();
        true
    }

    pub(crate) fn set_legacy_slack_sidebar_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if self.legacy_slack_sidebar_width == Some(width) {
            return;
        }
        self.legacy_slack_sidebar_width = Some(width);
        cx.notify();
    }

    pub(crate) fn resize_slack_sidebar_to_width(
        &mut self,
        width: f32,
        cx: &mut Context<Self>,
    ) -> bool {
        let (min_width, max_width) = self.slack_sidebar_width_bounds();
        let available_width = self.slack_sidebar_available_width();
        if available_width <= 0.0 {
            return false;
        }
        self.set_slack_sidebar_preferred_ratio(
            width.clamp(min_width, max_width) / available_width,
            cx,
        )
    }

    pub(crate) fn render_slack_sidebar(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let home_finder_active =
            self.slack_active_rail_view == SlackRailView::Home && self.slack_home_finder_active();
        (div()
            .w(px(self.slack_sidebar_width()))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.sidebar_bg))
            .border_r_1()
            .border_color(rgb(if self.slack_active_rail_view == SlackRailView::Dms {
                0x2f2f31
            } else {
                palette.sidebar_border
            }))
            .when(self.slack_active_rail_view == SlackRailView::Dms, |this| {
                this.child(self.render_slack_dm_sidebar(workspace, SlackDmSidebarMode::Full, cx))
            })
            .when(self.slack_active_rail_view != SlackRailView::Dms, |this| {
                this.child(self.render_slack_sidebar_header(workspace, cx))
                    .when(self.slack_active_rail_view == SlackRailView::Home, |this| {
                        this.when(!self.slack_upgrade_card_dismissed, |this| {
                            this.child(self.render_slack_upgrade_card(cx))
                        })
                        .child(self.render_slack_home_finder_search(cx))
                    })
                    .when(
                        self.slack_active_rail_view == SlackRailView::DraftsSent,
                        |this| this.child(self.render_slack_conversation_search_space()),
                    )
                    .when(home_finder_active, |this| {
                        this.child(self.render_slack_home_finder_results(cx))
                    })
                    .when(!home_finder_active, |this| {
                        this.child(self.render_slack_sidebar_sections(workspace, cx))
                    })
            }))
        .into_any_element()
    }

    pub(crate) fn render_slack_dm_peek_layer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available_height =
            (self.viewport_height - crate::ui::surface::SLACK_HISTORY_HEIGHT - SLACK_DM_PEEK_TOP)
                .max(0.0);
        let height = available_height.min(SLACK_DM_PEEK_MAX_HEIGHT);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .left(px(self.slack_rail_width()))
                .right(px(0.0))
                .top(px(0.0))
                .bottom(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_dms_peek(cx);
            }),
            cx,
        );
        div()
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .top(px(0.0))
            .bottom(px(0.0))
            .child(backdrop)
            .child(
                div()
                    .absolute()
                    .left(px(self.slack_rail_width() - 4.0))
                    .top(px(SLACK_DM_PEEK_TOP))
                    .w(px(SLACK_DM_PEEK_WIDTH))
                    .h(px(height))
                    .rounded(px(12.0))
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(0x343639))
                    .bg(rgb(0x222428))
                    .shadow(vec![BoxShadow {
                        color: alpha(0x000000, 0.34),
                        offset: point(px(0.0), px(10.0)),
                        blur_radius: px(28.0),
                        spread_radius: px(-8.0),
                        inset: false,
                    }])
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(self.render_slack_dm_sidebar(workspace, SlackDmSidebarMode::Peek, cx)),
            )
            .into_any_element()
    }

    pub(super) fn render_slack_sidebar_header(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_SIDEBAR_HEADER_HEIGHT))
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_workspace_identity(workspace, palette, cx))
            .child(self.render_slack_sidebar_header_actions(cx))
    }

    fn render_slack_conversation_search_space(&self) -> Div {
        div().h(px(32.0)).flex_shrink_0()
    }

    fn render_slack_workspace_identity(
        &self,
        workspace: &SlackWorkspace,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-workspace-menu")
            .role(Role::Button)
            .aria_label(format!("Switch workspaces… ({})", workspace.workspace_name))
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(palette.sidebar_icon, 0.12)))
            .focus_visible(|style| style.bg(alpha(palette.sidebar_icon, 0.12)))
            .flex()
            .items_center()
            .gap(px(6.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_workspace_panel(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_workspace_panel(cx);
                }
            }))
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BLACK)
                    .text_color(rgb(palette.sidebar_header_text))
                    .child(workspace.workspace_name.clone()),
            )
            .child(slack_icon(
                SlackShellIcon::ChevronDown,
                palette.sidebar_icon,
                18.0,
                cx,
            ))
    }

    fn render_slack_sidebar_header_actions(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w(px(76.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .justify_end()
            .gap(px(4.0))
            .child(div().size(px(36.0)))
            .when(
                self.slack_workspace_api_capabilities
                    .load_destination_directory,
                |this| {
                    this.child(
                        div()
                            .id("slack-workspace-compose")
                            .role(Role::Button)
                            .aria_label("New message")
                            .focusable()
                            .tab_stop(true)
                            .size(px(36.0))
                            .rounded(px(8.0))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|style| style.bg(alpha(palette.sidebar_icon, 0.12)))
                            .child(slack_icon(
                                SlackShellIcon::Compose,
                                palette.sidebar_icon,
                                20.0,
                                cx,
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                    this.compose_new_slack_message(cx);
                                }),
                            )
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                                    && !event.keystroke.modifiers.modified()
                                {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    this.compose_new_slack_message(cx);
                                }
                            })),
                    )
                },
            )
    }

    pub(super) fn render_slack_sidebar_sections(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let surface = cx.entity();
        let rows = self.current_slack_sidebar_rows(workspace);
        let viewport_bounds = self.slack_sidebar_list_state.viewport_bounds();
        let viewport_has_layout = viewport_bounds.size.height > px(0.0);
        let visible_row_range = if viewport_has_layout {
            self.slack_sidebar_visible_row_range(rows.len())
        } else {
            None
        };
        let (hidden_boundary_above, hidden_boundary_below) =
            slack_sidebar_boundary_targets(&rows, visible_row_range);

        div()
            .id("slack-home-sidebar-tree")
            .role(Role::Tree)
            .aria_label("Home sidebar")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .relative()
            .overflow_hidden()
            .px(px(8.0))
            .pt(px(6.0))
            .pb(px(20.0))
            .on_scroll_wheel(move |_, _, cx| {
                surface.update(cx, |surface, _| {
                    surface.cancel_slack_sidebar_active_row_reveal();
                    surface.mark_slack_remote_image_queue_dirty();
                });
            })
            .child(self.render_slack_sidebar_list_with_rows(rows, cx))
            .when_some(hidden_boundary_above, |this, target| {
                this.child(self.render_slack_sidebar_boundary_pill(target, cx))
            })
            .when_some(hidden_boundary_below, |this, target| {
                this.child(self.render_slack_sidebar_boundary_pill(target, cx))
            })
            .into_any_element()
    }

    fn current_slack_sidebar_rows(
        &self,
        workspace: &SlackWorkspace,
    ) -> std::sync::Arc<[SlackSidebarRow]> {
        if self.slack_sidebar_rows.is_empty() {
            build_slack_sidebar_rows(Some(workspace), &self.slack_collapsed_sections)
        } else {
            self.slack_sidebar_rows.clone()
        }
    }

    fn render_slack_sidebar_list_with_rows(
        &self,
        rows: std::sync::Arc<[SlackSidebarRow]>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        list(
            self.slack_sidebar_list_state.clone(),
            move |index, _window, cx| {
                let rows = rows.clone();
                view.update(cx, |this, cx| {
                    let row = rows
                        .get(index)
                        .expect("slack sidebar row index should exist");
                    this.render_slack_sidebar_row(row, cx)
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full()
        .into_any_element()
    }
}
