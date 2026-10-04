mod activity;
mod all_threads;
mod cached_views;
mod channel_details;
mod channel_menu;
mod date_jump;
mod directory;
mod drafts_sent;
mod files;
mod history;
mod later;
mod message_delete;
mod message_forward;
mod message_menu;
mod new_message;
mod rail;
mod rail_surfaces;
mod resize_drag;
mod search;
mod self_settings;
mod sidebar_section;

use resize_drag::DraggedSlackSidebarResize;

use super::{
    alpha, div, px, relative, rgb, slack_icon, slack_palette, AnyElement, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, ParentElement, SlackMainRoute, SlackRailView, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState, Window, SLACK_HISTORY_HEIGHT,
    SLACK_TOP_NAV_LEFT_INSET, SLACK_TOP_NAV_RIGHT_INSET,
};
use crate::ui::SlackWorkspace;
use gpui::{font, AnyView, Role, Stateful, StyleRefinement};

impl SurfaceState {
    fn render_cached_slack_history(&self) -> AnyElement {
        AnyView::from(self.slack_cached_history.clone())
            .cached(
                StyleRefinement::default()
                    .w_full()
                    .h(px(SLACK_HISTORY_HEIGHT)),
            )
            .into_any_element()
    }

    fn render_cached_slack_rail(&self) -> AnyElement {
        AnyView::from(self.slack_cached_rail.clone())
            .cached(
                StyleRefinement::default()
                    .w(px(self.slack_rail_width()))
                    .h_full(),
            )
            .into_any_element()
    }

    fn render_cached_slack_sidebar(&self) -> AnyElement {
        AnyView::from(self.slack_cached_sidebar.clone())
            .cached(
                StyleRefinement::default()
                    .w(px(self.slack_sidebar_width()))
                    .h_full(),
            )
            .into_any_element()
    }

    pub(super) fn render_cached_slack_conversation(&self) -> AnyElement {
        AnyView::from(self.slack_cached_conversation.clone())
            .cached(
                StyleRefinement::default()
                    .w_full()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .flex_grow(1.0),
            )
            .into_any_element()
    }

    fn render_slack_search_overlay(&self) -> AnyElement {
        AnyView::from(self.slack_search_overlay.clone()).into_any_element()
    }

    fn render_slack_search_results(&self) -> AnyElement {
        AnyView::from(self.slack_search_results.clone()).into_any_element()
    }

    fn render_slack_workspace_frame(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let frame = div()
            .id("slack-workspace-tabpanel")
            .role(Role::TabPanel)
            .aria_label(workspace.workspace_name.clone())
            .flex_grow(1.0)
            .min_w(px(0.0))
            .min_h(px(0.0))
            .mr(px(crate::ui::SLACK_WORKSPACE_FRAME_RIGHT_MARGIN))
            .mb(px(4.0))
            .relative()
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(palette.sidebar_border))
            .overflow_hidden()
            .flex();
        let frame = match self.slack_main_route {
            SlackMainRoute::Directory => frame
                .child(self.render_cached_slack_sidebar())
                .child(self.render_slack_directory_surface(cx)),
            SlackMainRoute::NewMessage => frame
                .child(self.render_cached_slack_sidebar())
                .child(self.render_slack_new_message_surface(workspace, cx)),
            SlackMainRoute::AllThreads => frame
                .child(self.render_cached_slack_sidebar())
                .child(self.render_slack_all_threads_surface(cx)),
            SlackMainRoute::Conversation => {
                self.render_slack_conversation_frame(frame, workspace, cx)
            }
        };
        frame.when(self.slack_workspace_frame_has_sidebar(), |this| {
            this.child(self.render_slack_sidebar_resize_handle(cx))
        })
    }

    fn slack_workspace_frame_has_sidebar(&self) -> bool {
        match self.slack_main_route {
            SlackMainRoute::Directory | SlackMainRoute::NewMessage | SlackMainRoute::AllThreads => {
                true
            }
            SlackMainRoute::Conversation => match self.slack_active_rail_view {
                SlackRailView::Activity
                | SlackRailView::Later
                | SlackRailView::More
                | SlackRailView::Admin => false,
                SlackRailView::Files => self.preview_width >= 1000.0,
                SlackRailView::DraftsSent => true,
                _ => !self.slack_search_results_open,
            },
        }
    }

    fn render_slack_conversation_frame(
        &self,
        frame: Stateful<Div>,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        match self.slack_active_rail_view {
            SlackRailView::Activity => frame.child(self.render_slack_activity_surface(cx)),
            SlackRailView::Later => frame.child(self.render_slack_later_surface(cx)),
            SlackRailView::Files => frame.child(self.render_slack_files_surface(cx)),
            SlackRailView::DraftsSent => frame
                .child(self.render_cached_slack_sidebar())
                .child(self.render_slack_drafts_sent_surface(cx)),
            SlackRailView::More => frame.child(self.render_slack_more_surface(cx)),
            SlackRailView::Admin => frame.child(self.render_slack_admin_surface(workspace, cx)),
            _ if self.slack_search_results_open => frame.child(self.render_slack_search_results()),
            _ => frame
                .child(self.render_cached_slack_sidebar())
                .child(self.render_slack_main(cx)),
        }
    }

    fn render_slack_workspace_body(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        let profile_panel = self.slack_profile_panel.clone();
        let thread_panel = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.origin.is_conversation())
            .cloned();
        let expanded_attachment = self.slack_expanded_attachment.clone();
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .relative()
            .flex()
            .child(self.render_cached_slack_rail())
            .child(self.render_slack_workspace_frame(workspace, cx))
            .when(self.slack_dms_peek_visible, |this| {
                this.child(self.render_slack_dm_peek_layer(workspace, cx))
            })
            .when_some(thread_panel, |this, panel| {
                this.child(self.render_slack_thread_panel(&panel, cx))
            })
            .when_some(profile_panel, |this, panel| {
                this.child(self.render_slack_profile_panel(&panel, cx))
            })
            .when_some(expanded_attachment, |this, selection| {
                this.child(self.render_slack_attachment_lightbox(&selection, cx))
            })
            .when(self.slack_rail_menu.is_some(), |this| {
                this.child(self.render_slack_rail_menu_layer(workspace, cx))
            })
            .when(self.slack_self_menu_open, |this| {
                this.child(self.render_slack_self_menu_layer(workspace, cx))
            })
    }

    fn render_slack_surface_layers(
        &self,
        surface: Div,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        surface
            .when(self.slack_search_open, |this| {
                this.child(self.render_slack_search_overlay())
            })
            .when(self.slack_history_menu_open, |this| {
                this.child(self.render_slack_history_menu_layer(cx))
            })
            .when(self.slack_conversation_tabs_overflow_open, |this| {
                this.child(self.render_slack_conversation_tabs_overflow_layer(cx))
            })
            .when(self.slack_channel_move_menu_open, |this| {
                this.child(self.render_slack_channel_move_menu_layer(cx))
            })
            .when(self.slack_channel_notifications_menu_open, |this| {
                this.child(self.render_slack_channel_notifications_menu_layer(cx))
            })
            .when(self.slack_channel_menu_open, |this| {
                this.child(self.render_slack_channel_menu_layer(cx))
            })
            .when(self.slack_message_menu.is_some(), |this| {
                this.child(self.render_slack_message_menu_layer(cx))
            })
            .when(self.slack_date_jump_overlay.is_some(), |this| {
                this.child(self.render_slack_date_jump_layer(cx))
            })
            .when(self.slack_message_forward_modal.is_some(), |this| {
                this.child(self.render_slack_message_forward_layer(cx))
            })
            .when(self.slack_message_delete_modal.is_some(), |this| {
                this.child(self.render_slack_message_delete_layer(cx))
            })
            .when(self.slack_self_status_dialog.is_some(), |this| {
                this.child(self.render_slack_self_status_layer(cx))
            })
            .when(self.slack_sidebar_section_dialog.is_some(), |this| {
                this.child(self.render_slack_sidebar_section_layer(cx))
            })
            .when(self.slack_channel_details_open, |this| {
                this.child(self.render_slack_channel_details_layer(workspace, cx))
            })
            .when(self.slack_schedule_overlay.is_some(), |this| {
                this.child(self.render_slack_schedule_overlay(cx))
            })
            .when(self.slack_members_panel_open, |this| {
                this.child(self.render_slack_members_layer(workspace, cx))
            })
            .when(self.slack_composer_link_dialog.is_some(), |this| {
                this.child(self.render_slack_composer_link_layer(cx))
            })
            .when_some(self.slack_video_clip_modal.as_ref(), |this, modal| {
                this.child(modal.clone())
            })
    }

    pub(crate) fn render_slack_surface(
        &mut self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let surface = div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .relative()
            .flex()
            .flex_col()
            .can_drop(|dragged, _, _| {
                dragged
                    .downcast_ref::<DraggedSlackSidebarResize>()
                    .is_some()
            })
            .on_drag_move::<DraggedSlackSidebarResize>(
                cx.listener(Self::handle_slack_sidebar_resize_drag_move),
            )
            .on_drop(cx.listener(
                |this: &mut SurfaceState, _: &DraggedSlackSidebarResize, _, cx| {
                    this.commit_slack_sidebar_width(cx);
                },
            ))
            .bg(rgb(palette.surface_bg))
            .text_color(rgb(palette.surface_text))
            .font(font("Lato"))
            .child(self.render_cached_slack_history())
            .child(self.render_slack_workspace_body(workspace, cx));
        self.render_slack_surface_layers(surface, workspace, cx)
            .into_any_element()
    }

    pub(crate) fn render_slack_root(
        &mut self,
        workspace: &SlackWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.schedule_slack_conversation_load_profile_finish(
            &workspace.conversation_id,
            window,
            cx,
        );
        let surface = self.render_slack_surface(workspace, cx);
        (div()
            .id("notslack-root")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .focusable()
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(surface))
        .into_any_element()
    }
}
