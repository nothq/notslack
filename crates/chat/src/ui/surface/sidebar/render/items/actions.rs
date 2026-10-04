use super::{
    div, img, px, rgb, slack_palette, AnyElement, AppearanceMode, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Styled, SurfaceState,
};
use crate::ui::surface::{
    SlackSidebarBoundaryDirection, SlackSidebarBoundaryLabel, SlackSidebarBoundaryTarget,
};
use crate::ui::{alpha, svg_from_body, SlackSidebarItem};
use gpui::{point, BoxShadow, Image, Role, StatefulInteractiveElement};
use std::sync::{Arc, OnceLock};

struct SlackSidebarBoundaryPillSpec {
    element_id: &'static str,
    accessibility_label: &'static str,
    label: &'static str,
    points_up: bool,
    left: f32,
    background: gpui::Hsla,
    hover_background: gpui::Hsla,
    text: u32,
}

static DARK_MENTIONS_UP_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static DARK_MENTIONS_DOWN_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static DARK_UNREADS_UP_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static DARK_UNREADS_DOWN_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static LIGHT_MENTIONS_UP_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static LIGHT_MENTIONS_DOWN_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static LIGHT_UNREADS_UP_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
static LIGHT_UNREADS_DOWN_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();

impl SurfaceState {
    pub(crate) fn render_slack_sidebar_separator(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w_full()
            .h(px(24.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .child(div().w_full().h(px(1.0)).bg(rgb(palette.sidebar_border)))
    }

    pub(crate) fn render_slack_drop_hint(&self) -> Div {
        div().w_full().h(px(44.0))
    }

    pub(crate) fn render_slack_sidebar_boundary_pill(
        &self,
        target: SlackSidebarBoundaryTarget,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let spec = slack_sidebar_boundary_pill_spec(target, &palette);
        let pill_view = div()
            .id(spec.element_id)
            .role(Role::Button)
            .aria_label(spec.accessibility_label)
            .focusable()
            .tab_stop(true)
            .absolute()
            .left(px(spec.left))
            .when(spec.points_up, |this| this.top(px(6.0)))
            .when(!spec.points_up, |this| this.bottom(px(4.0)))
            .h(px(28.0))
            .py(px(4.0))
            .pl(px(8.0))
            .pr(px(20.0))
            .rounded(px(16.0))
            .bg(spec.background)
            .cursor_pointer()
            .hover(move |style| style.bg(spec.hover_background))
            .focus_visible(move |style| style.bg(spec.hover_background));
        self.bind_slack_sidebar_boundary_pill(pill_view, target, cx)
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.08),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(12.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .text_size(px(13.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(spec.text))
            .child(
                img(slack_sidebar_boundary_arrow_image(
                    target,
                    self.appearance_mode,
                    spec.text,
                ))
                .size(px(13.0)),
            )
            .child(spec.label)
    }

    fn bind_slack_sidebar_boundary_pill(
        &self,
        pill_view: gpui::Stateful<Div>,
        target: SlackSidebarBoundaryTarget,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        pill_view
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.scroll_to_slack_sidebar_boundary(target, window, cx);
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                        || event.keystroke.modifiers.modified()
                    {
                        return;
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    this.scroll_to_slack_sidebar_boundary(target, window, cx);
                }),
            )
    }

    pub(crate) fn bind_slack_sidebar_item_click(
        &self,
        row: Div,
        item: &SlackSidebarItem,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = row
            .id(format!("slack-sidebar-tree-item-{:p}", item))
            .role(Role::TreeItem)
            .aria_label(item.label.clone())
            .aria_selected(item.active);
        if item.target_id.is_empty() || !self.slack_workspace_api_capabilities.load_conversation {
            return row.into_any_element();
        }
        let conversation_id = item.target_id.clone();
        let keyboard_conversation_id = conversation_id.clone();
        row.focusable()
            .tab_stop(true)
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.select_slack_conversation(&conversation_id, cx);
                }),
            )
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.select_slack_conversation(&keyboard_conversation_id, cx);
                    }
                }),
            )
            .into_any_element()
    }
}

fn slack_sidebar_boundary_pill_spec(
    target: SlackSidebarBoundaryTarget,
    palette: &super::super::super::SlackPalette,
) -> SlackSidebarBoundaryPillSpec {
    let (background, hover_background, text) = match target.label {
        SlackSidebarBoundaryLabel::UnreadMentions => (
            rgb(palette.sidebar_badge_bg).into(),
            rgb(palette.sidebar_badge_bg).into(),
            palette.sidebar_badge_text,
        ),
        SlackSidebarBoundaryLabel::MoreUnreads => (
            rgb(palette.sidebar_more_unreads_bg).into(),
            rgb(palette.sidebar_more_unreads_hover_bg).into(),
            palette.sidebar_more_unreads_text,
        ),
    };
    let (element_id, accessibility_label) = match (target.direction, target.label) {
        (SlackSidebarBoundaryDirection::Above, SlackSidebarBoundaryLabel::UnreadMentions) => {
            ("slack-unread-mentions-above", "Show unread mentions above")
        }
        (SlackSidebarBoundaryDirection::Above, SlackSidebarBoundaryLabel::MoreUnreads) => {
            ("slack-more-unreads-above", "Show more unreads above")
        }
        (SlackSidebarBoundaryDirection::Below, SlackSidebarBoundaryLabel::UnreadMentions) => {
            ("slack-unread-mentions-below", "Show unread mentions below")
        }
        (SlackSidebarBoundaryDirection::Below, SlackSidebarBoundaryLabel::MoreUnreads) => {
            ("slack-more-unreads-below", "Show more unreads below")
        }
    };
    SlackSidebarBoundaryPillSpec {
        element_id,
        accessibility_label,
        label: target.label.text(),
        points_up: target.direction == SlackSidebarBoundaryDirection::Above,
        left: 112.0,
        background,
        hover_background,
        text,
    }
}

fn slack_sidebar_boundary_arrow_image(
    target: SlackSidebarBoundaryTarget,
    appearance_mode: AppearanceMode,
    fill: u32,
) -> Arc<Image> {
    let cache = slack_sidebar_boundary_arrow_cache(target, appearance_mode);
    let path = if target.direction == SlackSidebarBoundaryDirection::Above {
        "M10.543 3.232a.75.75 0 0 0-1.086 0l-5.25 5.5a.75.75 0 0 0 1.086 1.036L9.25 5.622V16.25a.75.75 0 0 0 1.5 0V5.622l3.957 4.146a.75.75 0 0 0 1.085-1.036z"
    } else {
        "M10.75 3.75a.75.75 0 0 0-1.5 0v10.628l-3.957-4.146a.75.75 0 0 0-1.086 1.036l5.25 5.5a.75.75 0 0 0 1.085 0l5.25-5.5a.75.75 0 0 0-1.085-1.036l-3.957 4.146z"
    };
    cache
        .get_or_init(|| {
            svg_from_body(
                "0 0 20 20",
                format!(r##"<path fill="#{fill:06x}" d="{path}"/>"##),
            )
        })
        .clone()
}

fn slack_sidebar_boundary_arrow_cache(
    target: SlackSidebarBoundaryTarget,
    appearance_mode: AppearanceMode,
) -> &'static OnceLock<Arc<Image>> {
    match (appearance_mode, target.direction, target.label) {
        (
            AppearanceMode::Dark,
            SlackSidebarBoundaryDirection::Above,
            SlackSidebarBoundaryLabel::UnreadMentions,
        ) => &DARK_MENTIONS_UP_IMAGE,
        (
            AppearanceMode::Dark,
            SlackSidebarBoundaryDirection::Below,
            SlackSidebarBoundaryLabel::UnreadMentions,
        ) => &DARK_MENTIONS_DOWN_IMAGE,
        (
            AppearanceMode::Dark,
            SlackSidebarBoundaryDirection::Above,
            SlackSidebarBoundaryLabel::MoreUnreads,
        ) => &DARK_UNREADS_UP_IMAGE,
        (
            AppearanceMode::Dark,
            SlackSidebarBoundaryDirection::Below,
            SlackSidebarBoundaryLabel::MoreUnreads,
        ) => &DARK_UNREADS_DOWN_IMAGE,
        (
            AppearanceMode::Light,
            SlackSidebarBoundaryDirection::Above,
            SlackSidebarBoundaryLabel::UnreadMentions,
        ) => &LIGHT_MENTIONS_UP_IMAGE,
        (
            AppearanceMode::Light,
            SlackSidebarBoundaryDirection::Below,
            SlackSidebarBoundaryLabel::UnreadMentions,
        ) => &LIGHT_MENTIONS_DOWN_IMAGE,
        (
            AppearanceMode::Light,
            SlackSidebarBoundaryDirection::Above,
            SlackSidebarBoundaryLabel::MoreUnreads,
        ) => &LIGHT_UNREADS_UP_IMAGE,
        (
            AppearanceMode::Light,
            SlackSidebarBoundaryDirection::Below,
            SlackSidebarBoundaryLabel::MoreUnreads,
        ) => &LIGHT_UNREADS_DOWN_IMAGE,
    }
}
