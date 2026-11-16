//! The profile card shown while the pointer rests on a name or mention.

use gpui::prelude::FluentBuilder;
use gpui::{
    anchored, deferred, div, img, point, px, rgb, AnyElement, BoxShadow, Context, FontWeight,
    IntoElement, ParentElement, Styled,
};

use crate::ui::surface::{
    profile::slack_profile_local_time, slack_icon, SlackShellIcon, SurfaceState,
};
use crate::ui::{alpha, initials, slack_avatar_fill, SlackUserPresence};

const SLACK_HOVER_CARD_WIDTH: f32 = 340.0;
const SLACK_HOVER_CARD_AVATAR: f32 = 96.0;

impl SurfaceState {
    pub(super) fn render_slack_hover_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let card = self.slack_hover_card.as_ref().filter(|card| card.visible)?;
        let profile = self.slack_profile_cache.get(&card.user_id);
        let name = profile.map_or_else(
            || "Loading…".to_string(),
            |profile| profile.real_name.clone(),
        );
        let avatar = profile
            .and_then(|profile| profile.avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        let presence = self.slack_workspace().and_then(|workspace| {
            self.slack_presence_authority
                .resolve_presence(&workspace.team_id, &card.user_id, None)
        });
        let status = profile.and_then(slack_hover_card_status);
        let local_time = profile
            .and_then(|profile| profile.timezone_id.as_deref())
            .and_then(slack_profile_local_time);
        let avatar_element = slack_hover_card_avatar(avatar, &name);
        let details = status.is_some() || local_time.is_some();
        let body = div()
            .w(px(SLACK_HOVER_CARD_WIDTH))
            .rounded(px(10.0))
            .border_1()
            .border_color(alpha(0xffffff, 0.12))
            .bg(rgb(0x222529))
            .shadow(slack_hover_card_shadow())
            .flex()
            .flex_col()
            .child(
                div()
                    .p(px(20.0))
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(avatar_element)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(17.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child(name),
                            )
                            .when_some(presence, |this, presence| {
                                let icon = match presence {
                                    SlackUserPresence::Active => SlackShellIcon::PresenceActive,
                                    SlackUserPresence::Away => SlackShellIcon::PresenceAway,
                                };
                                this.child(slack_icon(icon, 0x1d9bd1, 12.0, cx))
                            }),
                    ),
            )
            .when(details, |this| {
                this.child(div().h(px(1.0)).bg(alpha(0xffffff, 0.10)))
                    .child(
                        div()
                            .px(px(20.0))
                            .py(px(14.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .text_size(px(15.0))
                            .text_color(rgb(0xd1d2d3))
                            .when_some(status, |this, status| this.child(div().child(status)))
                            .when_some(local_time, |this, time| {
                                this.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(slack_icon(
                                            SlackShellIcon::Clock,
                                            0xd1d2d3,
                                            16.0,
                                            cx,
                                        ))
                                        .child(format!("{time} local time")),
                                )
                            }),
                    )
            });
        Some(
            deferred(
                anchored()
                    .position(card.position + point(px(12.0), px(18.0)))
                    .snap_to_window()
                    .child(body),
            )
            .with_priority(60)
            .into_any_element(),
        )
    }
}

fn slack_hover_card_status(profile: &crate::ui::SlackProfile) -> Option<String> {
    let text = profile.status_text.clone()?;
    Some(
        match profile
            .status_emoji
            .as_deref()
            .and_then(|code| emojis::get_by_shortcode(code.trim_matches(':')))
        {
            Some(emoji) => format!("{} {text}", emoji.as_str()),
            None => text,
        },
    )
}

fn slack_hover_card_avatar(avatar: Option<std::sync::Arc<gpui::Image>>, name: &str) -> AnyElement {
    match avatar {
        Some(image) => img(image)
            .size(px(SLACK_HOVER_CARD_AVATAR))
            .rounded(px(8.0))
            .into_any_element(),
        None => div()
            .size(px(SLACK_HOVER_CARD_AVATAR))
            .rounded(px(8.0))
            .bg(rgb(slack_avatar_fill(name)))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(32.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(initials(name))
            .into_any_element(),
    }
}

fn slack_hover_card_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(0x000000, 0.4),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(0.0),
        inset: false,
    }]
}
