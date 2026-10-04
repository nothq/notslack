use super::{
    div, img, px, rgb, slack_base_icon_radius, slack_icon, slack_palette, AnyElement,
    AppearanceMode, Arc, Context, Div, FluentBuilder, FontWeight, HashMap, Image, IntoElement,
    ParentElement, SlackPalette, SlackShellIcon, Styled, SurfaceState,
};
use crate::ui::surface::channel::{
    slack_avatar_presence_badge, SlackAvatarPresenceBadgeSize, SlackAvatarPresenceBadgeSpec,
};
use crate::ui::{initials, slack_avatar_fill, slack_sidebar_icon, svg_from_body};
use crate::ui::{SlackConversationKind, SlackSidebarItem, SlackUserPresence};
use gpui::SharedString;
use std::sync::OnceLock;

pub(super) struct SlackSidebarItemLeadingSpec<'a> {
    pub(super) item: &'a SlackSidebarItem,
    pub(super) active: bool,
    pub(super) label: &'a str,
    pub(super) group_count_label: Option<SharedString>,
    pub(super) has_avatar: bool,
    pub(super) peer_notifications_paused: bool,
    pub(super) appearance_mode: AppearanceMode,
    pub(super) remote_images: &'a HashMap<String, Arc<Image>>,
}

#[derive(Clone, Copy)]
struct SlackSidebarPresenceStyle {
    is_external_connection: bool,
    active: bool,
    peer_notifications_paused: bool,
    appearance_mode: AppearanceMode,
}

pub(super) fn slack_sidebar_item_leading(
    spec: SlackSidebarItemLeadingSpec<'_>,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let palette = slack_palette(spec.appearance_mode);
    if let Some(avatar) = slack_sidebar_avatar_leading(&spec, cx) {
        return avatar;
    }

    let icon_color = slack_sidebar_leading_icon_color(spec.item, spec.active, &palette);
    match spec.item.icon.as_deref() {
        Some("hash") => {
            let icon = if spec.item.unread {
                SlackShellIcon::ChannelFilled
            } else {
                SlackShellIcon::HashSmall
            };
            slack_icon(icon, icon_color, 16.0, cx)
        }
        Some("lock") => slack_icon(SlackShellIcon::LockSmall, icon_color, 16.0, cx),
        Some("person") if spec.item.target_kind == SlackConversationKind::GroupMessage => {
            slack_sidebar_group_message_badge(
                spec.active,
                spec.group_count_label
                    .expect("Slack group message row must have a participant count"),
                spec.appearance_mode,
            )
            .into_any_element()
        }
        Some("person") => slack_icon(SlackShellIcon::People, icon_color, 13.0, cx),
        _ => slack_sidebar_fallback_icon(spec.item, icon_color).into_any_element(),
    }
}

fn slack_sidebar_avatar_leading(
    spec: &SlackSidebarItemLeadingSpec<'_>,
    cx: &mut Context<SurfaceState>,
) -> Option<AnyElement> {
    if !spec.has_avatar {
        return None;
    }
    let presence_style = SlackSidebarPresenceStyle {
        is_external_connection: spec.item.is_external_connection,
        active: spec.active,
        peer_notifications_paused: spec.peer_notifications_paused,
        appearance_mode: spec.appearance_mode,
    };
    let image = spec
        .item
        .avatar_image_url
        .as_deref()
        .and_then(|url| spec.remote_images.get(url).cloned());
    Some(if let Some(image) = image {
        slack_sidebar_avatar_image(image, spec.label, spec.item.presence, presence_style, cx)
    } else {
        slack_sidebar_avatar_placeholder(spec.label, spec.item.presence, presence_style, cx)
            .into_any_element()
    })
}

fn slack_sidebar_avatar_image(
    image: Arc<Image>,
    label: &str,
    presence: Option<SlackUserPresence>,
    presence_style: SlackSidebarPresenceStyle,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    slack_sidebar_avatar_presence(
        div()
            .relative()
            .size(px(16.0))
            .rounded(slack_base_icon_radius(16.0))
            .bg(rgb(slack_avatar_fill(label)))
            .child(
                div()
                    .size_full()
                    .rounded(slack_base_icon_radius(16.0))
                    .overflow_hidden()
                    .child(
                        img(image)
                            .w_full()
                            .h_full()
                            .rounded(slack_base_icon_radius(16.0)),
                    ),
            ),
        presence,
        presence_style,
        cx,
    )
    .into_any_element()
}

fn slack_sidebar_avatar_placeholder(
    label: &str,
    presence: Option<SlackUserPresence>,
    presence_style: SlackSidebarPresenceStyle,
    cx: &mut Context<SurfaceState>,
) -> Div {
    slack_sidebar_avatar_presence(
        div()
            .relative()
            .size(px(16.0))
            .rounded(slack_base_icon_radius(16.0))
            .bg(rgb(slack_avatar_fill(label)))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(8.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xffffff))
            .child(initials(label)),
        presence,
        presence_style,
        cx,
    )
}

fn slack_sidebar_avatar_presence(
    avatar: Div,
    presence: Option<SlackUserPresence>,
    style: SlackSidebarPresenceStyle,
    cx: &mut Context<SurfaceState>,
) -> Div {
    if style.is_external_connection {
        if style.peer_notifications_paused {
            return avatar
                .child(slack_external_sidebar_dnd_avatar_overlay(style))
                .child(slack_external_sidebar_dnd_presence_badge(style, cx));
        }
        return avatar.when_some(presence, |this, presence| {
            this.child(slack_external_sidebar_presence_badge(presence, style))
        });
    }

    let palette = slack_palette(style.appearance_mode);
    let background = if style.active {
        palette.sidebar_active_bg
    } else {
        palette.sidebar_bg
    };
    let (active_color, away_color) = if style.active {
        (palette.sidebar_active_text, palette.sidebar_active_text)
    } else {
        (0x20a271, palette.sidebar_text)
    };
    avatar.when_some(
        slack_avatar_presence_badge(
            SlackAvatarPresenceBadgeSpec {
                size: SlackAvatarPresenceBadgeSize::Sidebar16,
                presence,
                notifications_paused: style.peer_notifications_paused,
                background,
                active_color,
                away_color,
            },
            cx,
        ),
        |this, badge| this.child(badge),
    )
}

fn slack_external_sidebar_dnd_avatar_overlay(style: SlackSidebarPresenceStyle) -> AnyElement {
    static DARK_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    static LIGHT_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();

    let fill = slack_palette(style.appearance_mode).sidebar_active_bg;
    let image = match style.appearance_mode {
        AppearanceMode::Dark => {
            DARK_IMAGE.get_or_init(|| slack_external_sidebar_dnd_overlay_image(fill))
        }
        AppearanceMode::Light => {
            LIGHT_IMAGE.get_or_init(|| slack_external_sidebar_dnd_overlay_image(fill))
        }
    };
    img(image.clone())
        .absolute()
        .left(px(0.0))
        .top(px(0.0))
        .size(px(16.0))
        .into_any_element()
}

fn slack_external_sidebar_dnd_overlay_image(fill: u32) -> Arc<Image> {
    svg_from_body(
        "0 0 16 16",
        format!(
            r##"<path fill="#{fill:06x}" d="M16 7.104H15.952C15.008 7.104 14.208 7.68 13.872 8.496C12.032 9.072 10.704 10.784 10.704 12.8C10.704 14.048 11.216 15.184 12.032 16H16Z"/>"##
        ),
    )
}

fn slack_external_sidebar_dnd_presence_badge(
    style: SlackSidebarPresenceStyle,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let palette = slack_palette(style.appearance_mode);
    div()
        .absolute()
        .right(px(-5.2))
        .bottom(px(-2.8))
        .size(px(12.0))
        .flex()
        .items_center()
        .justify_center()
        .child(slack_icon(
            SlackShellIcon::PresenceDnd,
            palette.sidebar_active_text,
            12.0,
            cx,
        ))
}

fn slack_external_sidebar_presence_badge(
    presence: SlackUserPresence,
    style: SlackSidebarPresenceStyle,
) -> Div {
    let palette = slack_palette(style.appearance_mode);
    let color = match presence {
        SlackUserPresence::Active => 0x20a271,
        SlackUserPresence::Away => {
            if style.active {
                palette.sidebar_active_text
            } else {
                palette.sidebar_icon
            }
        }
    };
    div()
        .absolute()
        .right(px(-5.0))
        .bottom(px(-3.0))
        .size(px(12.0))
        .flex()
        .items_center()
        .justify_center()
        .child(div().size(px(5.0)).rounded(px(1.0)).when_else(
            presence == SlackUserPresence::Active,
            |this| this.bg(rgb(color)),
            |this| this.border_1().border_color(rgb(color)),
        ))
}

fn slack_sidebar_leading_icon_color(
    item: &SlackSidebarItem,
    active: bool,
    palette: &SlackPalette,
) -> u32 {
    if active {
        palette.sidebar_active_text
    } else if item.unread {
        palette.sidebar_unread_text
    } else {
        palette.sidebar_icon
    }
}

fn slack_sidebar_fallback_icon(item: &SlackSidebarItem, color: u32) -> Div {
    div()
        .w(px(12.0))
        .text_size(px(14.0))
        .text_color(rgb(color))
        .child(slack_sidebar_icon(item.icon.as_deref()))
}

fn slack_sidebar_group_message_badge(
    active: bool,
    count_label: SharedString,
    appearance_mode: AppearanceMode,
) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .size(px(16.0))
        .rounded(px(4.0))
        .bg(if active {
            rgb(palette.sidebar_active_bg)
        } else {
            rgb(0xd1d2d3)
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(if active {
            palette.sidebar_active_text
        } else {
            0x1a1d21
        }))
        .child(count_label)
}

pub(super) fn slack_sidebar_item_text_color(
    item: &SlackSidebarItem,
    active: bool,
    unread: bool,
    appearance_mode: AppearanceMode,
) -> u32 {
    let palette = slack_palette(appearance_mode);
    if active {
        palette.sidebar_active_text
    } else if unread {
        palette.sidebar_unread_text
    } else if item.muted {
        palette.sidebar_muted_text
    } else {
        palette.sidebar_text
    }
}

pub(crate) fn slack_sidebar_item_badge(
    _item: &SlackSidebarItem,
    count: u32,
    appearance_mode: AppearanceMode,
) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .min_w(px(24.0))
        .h(px(18.0))
        .px(px(9.0))
        .rounded_full()
        .bg(rgb(palette.sidebar_badge_bg))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .line_height(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(palette.sidebar_badge_text))
        .child(count.to_string())
}
