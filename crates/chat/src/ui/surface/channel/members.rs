use super::{
    alpha, div, img, point, px, relative, rgb, slack_base_icon_radius, slack_icon, slack_palette,
    AnyElement, BoxShadow, Context, FluentBuilder, FontWeight, Image, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement,
    SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::{svg_from_body, SlackUserPresence, SlackWorkspace};
use gpui::Role;
use gpui_components::backdrop::blocking_backdrop;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

const SLACK_MEMBERS_DIALOG_WIDTH: f32 = 580.0;
const SLACK_MEMBERS_DIALOG_HEIGHT: f32 = 820.0;
const SLACK_MEMBERS_ROW_HEIGHT: f32 = 60.0;

const SLACK_SMALL_AVATAR_PRESENCE_OVERLAY: &str =
    "M1 .523 C.984 .52 .967 .519 .95 .519 C.795 .519 .669 .645 .669 .8 C.669 .878 .701 .949 .752 1 H1 Z";
const SLACK_SMALL_AVATAR_DND_OVERLAY: &str =
    "M1 .444 H.997 C.938 .444 .888 .48 .867 .531 C.752 .567 .669 .674 .669 .8 C.669 .878 .701 .949 .752 1 H1 Z";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::ui::surface) enum SlackAvatarPresenceBadgeSize {
    Sidebar16,
    Header24,
}

pub(in crate::ui::surface) struct SlackAvatarPresenceBadgeSpec {
    pub(in crate::ui::surface) size: SlackAvatarPresenceBadgeSize,
    pub(in crate::ui::surface) presence: Option<SlackUserPresence>,
    pub(in crate::ui::surface) notifications_paused: bool,
    pub(in crate::ui::surface) background: u32,
    pub(in crate::ui::surface) active_color: u32,
    pub(in crate::ui::surface) away_color: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SlackAvatarPresenceOverlayKey {
    size: SlackAvatarPresenceBadgeSize,
    dnd: bool,
    background: u32,
}

type SlackAvatarPresenceOverlayCache = Mutex<HashMap<SlackAvatarPresenceOverlayKey, Arc<Image>>>;

static SLACK_AVATAR_PRESENCE_OVERLAY_CACHE: OnceLock<SlackAvatarPresenceOverlayCache> =
    OnceLock::new();

mod header;
mod list;

impl SurfaceState {
    pub(crate) fn render_slack_members_layer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog_height = (self.viewport_height - 24.0).clamp(320.0, SLACK_MEMBERS_DIALOG_HEIGHT);
        let dialog_top = ((self.viewport_height - dialog_height) / 2.0 + 6.5).max(12.0);
        let layer = div()
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .top(px(0.0))
            .bottom(px(0.0))
            .bg(alpha(0x000000, 0.55))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(dialog_top))
            .child(
                div()
                    .id("slack-channel-members-dialog")
                    .role(Role::Dialog)
                    .aria_label(format!("Details for channel #{}", workspace.channel_name))
                    .w(px(SLACK_MEMBERS_DIALOG_WIDTH))
                    .h(px(dialog_height))
                    .max_w(relative(0.92))
                    .rounded(px(8.0))
                    .overflow_hidden()
                    .occlude()
                    .bg(rgb(slack_palette(self.appearance_mode).main_bg))
                    .shadow(vec![BoxShadow {
                        color: alpha(0x000000, 0.28),
                        offset: point(px(0.0), px(8.0)),
                        blur_radius: px(28.0),
                        spread_radius: px(0.0),
                        inset: false,
                    }])
                    .flex()
                    .flex_col()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(self.render_slack_members_dialog_header(workspace, cx))
                    .child(self.render_slack_members_dialog_search(cx))
                    .child(self.render_slack_members_dialog_content(cx)),
            );
        blocking_backdrop(layer, cx).into_any_element()
    }

    fn render_slack_members_dialog_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(68.0))
            .flex_none()
            .px(px(28.0))
            .flex()
            .items_center()
            .child(
                div()
                    .h(px(38.0))
                    .w_full()
                    .rounded(px(8.0))
                    .border_2()
                    .border_color(rgb(0x1264a3))
                    .shadow(vec![BoxShadow {
                        color: alpha(0x1d9bd1, 0.28),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(3.0),
                        inset: false,
                    }])
                    .px(px(11.0))
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .child(slack_icon(
                        SlackShellIcon::Search,
                        palette.main_text,
                        20.0,
                        cx,
                    ))
                    .child(
                        div()
                            .flex_grow(1.0)
                            .min_w(px(0.0))
                            .h_full()
                            .child(self.slack_members_search_input_entity(cx)),
                    ),
            )
    }

    fn render_slack_members_skeleton(&self) -> impl IntoElement {
        let fill = if self.appearance_mode == crate::ui::AppearanceMode::Dark {
            0x34363b
        } else {
            0xe8e8e8
        };
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .children((0..8).map(|index| {
                div()
                    .h(px(SLACK_MEMBERS_ROW_HEIGHT))
                    .flex_none()
                    .px(px(28.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(slack_base_icon_radius(36.0))
                            .bg(rgb(fill)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .w(px(104.0 + (index % 3) as f32 * 24.0))
                                    .h(px(10.0))
                                    .rounded(px(5.0))
                                    .bg(rgb(fill)),
                            )
                            .child(
                                div()
                                    .w(px(72.0 + (index % 4) as f32 * 18.0))
                                    .h(px(8.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(fill)),
                            ),
                    )
            }))
    }
}

pub(in crate::ui::surface) fn slack_members_presence_dot(
    presence: SlackUserPresence,
    row_background: u32,
) -> impl IntoElement {
    match presence {
        SlackUserPresence::Active => div().size(px(8.0)).rounded_full().bg(rgb(0x20a271)),
        SlackUserPresence::Away => div()
            .size(px(8.0))
            .rounded_full()
            .border_2()
            .border_color(rgb(0x616061))
            .bg(rgb(row_background)),
    }
}

pub(in crate::ui::surface) fn slack_avatar_presence_badge(
    spec: SlackAvatarPresenceBadgeSpec,
    cx: &mut Context<SurfaceState>,
) -> Option<AnyElement> {
    let (icon, color) = match (spec.presence, spec.notifications_paused) {
        (Some(SlackUserPresence::Active), false) => {
            (SlackShellIcon::PresenceActive, spec.active_color)
        }
        (Some(SlackUserPresence::Away), false) => (SlackShellIcon::PresenceAway, spec.away_color),
        (None, false) => return None,
        (Some(SlackUserPresence::Active), true) => {
            (SlackShellIcon::PresenceDndFilled, spec.active_color)
        }
        (Some(SlackUserPresence::Away), true) | (None, true) => {
            (SlackShellIcon::PresenceDnd, spec.away_color)
        }
    };
    let (avatar_size, icon_size, right, bottom) = spec.size.geometry();
    let overlay = slack_avatar_presence_overlay_image(SlackAvatarPresenceOverlayKey {
        size: spec.size,
        dnd: spec.notifications_paused,
        background: spec.background,
    });
    Some(
        div()
            .absolute()
            .left(px(0.0))
            .top(px(0.0))
            .size(px(avatar_size))
            .child(
                img(overlay)
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .size(px(avatar_size)),
            )
            .child(
                div()
                    .absolute()
                    .right(px(right))
                    .bottom(px(bottom))
                    .size(px(icon_size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(icon, color, icon_size, cx)),
            )
            .into_any_element(),
    )
}

impl SlackAvatarPresenceBadgeSize {
    fn geometry(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Sidebar16 => (16.0, 12.0, -5.2, -2.8),
            Self::Header24 => (24.0, 18.0, -7.8, -4.2),
        }
    }
}

fn slack_avatar_presence_overlay_image(key: SlackAvatarPresenceOverlayKey) -> Arc<Image> {
    let mut cache = SLACK_AVATAR_PRESENCE_OVERLAY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Slack avatar presence overlay cache mutex poisoned");
    cache
        .entry(key)
        .or_insert_with(|| {
            let path = if key.dnd {
                SLACK_SMALL_AVATAR_DND_OVERLAY
            } else {
                SLACK_SMALL_AVATAR_PRESENCE_OVERLAY
            };
            svg_from_body(
                "0 0 1 1",
                format!(r##"<path fill="#{:06x}" d="{path}"/>"##, key.background),
            )
        })
        .clone()
}

fn slack_members_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
