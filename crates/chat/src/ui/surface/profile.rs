use super::{
    alpha, div, img, px, relative, rgb, AnyElement, Arc, Context, Div, FluentBuilder, FontWeight,
    Image, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackProfilePanelState, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_PROFILE_PANEL_WIDTH,
};
use super::{slack_icon, SlackShellIcon};
use crate::ui::initials;
use crate::ui::slack_avatar_fill;
use crate::ui::{SlackProfile, SlackUserPresence};

struct SlackProfileBodyConfig {
    title: String,
    avatar_label: String,
    avatar_fill: u32,
    avatar_image: Option<Arc<Image>>,
}

const SLACK_PROFILE_AVATAR_SIZE: f32 = 256.0;
const SLACK_PROFILE_AVATAR_RADIUS: f32 = 8.0;

impl SurfaceState {
    pub(crate) fn render_slack_profile_panel(
        &self,
        panel: &SlackProfilePanelState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = panel.label().to_string();
        let avatar_label = slack_profile_avatar_label(panel, &title);
        let avatar_fill = slack_avatar_fill(&title);
        let avatar_image = slack_profile_avatar_image(panel)
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        let body = SlackProfileBodyConfig {
            title: title.clone(),
            avatar_label,
            avatar_fill,
            avatar_image,
        };
        div()
            .absolute()
            .right(px(0.0))
            .top(px(0.0))
            .child(
                (div()
                    .w(px(SLACK_PROFILE_PANEL_WIDTH))
                    .h_full()
                    .border_l_1()
                    .border_color(alpha(0xffffff, 0.10))
                    .bg(rgb(0x19181a))
                    .flex()
                    .flex_col()
                    .child(self.render_slack_profile_header(cx))
                    .child(self.render_slack_profile_body(panel, body, cx)))
                .into_any_element(),
            )
            .into_any_element()
    }

    fn render_slack_profile_header(&self, cx: &mut Context<Self>) -> AnyElement {
        (div()
            .h(px(56.0))
            .px(px(16.0))
            .border_b_1()
            .border_color(alpha(0xffffff, 0.08))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Profile"),
            )
            .child(self.render_slack_profile_close_button(cx)))
        .into_any_element()
    }

    fn render_slack_profile_close_button(&self, cx: &mut Context<Self>) -> Div {
        div()
            .size(px(28.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.close_slack_profile_panel(cx);
                }),
            )
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .child(slack_icon(SlackShellIcon::Close, 0xc8cbd0, 20.0, cx))
    }

    fn render_slack_profile_body(
        &self,
        panel: &SlackProfilePanelState,
        body: SlackProfileBodyConfig,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("slack-profile-scroll")
            .overflow_scroll()
            .track_scroll(&self.slack_profile_scroll_handle)
            .px(px(20.0))
            .pt(px(20.0))
            .pb(px(18.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(slack_profile_avatar(
                &body.avatar_label,
                body.avatar_fill,
                body.avatar_image,
            ))
            .child(self.render_slack_profile_state(panel, &body.title, cx))
            .into_any_element()
    }

    fn render_slack_profile_state(
        &self,
        panel: &SlackProfilePanelState,
        title: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match panel {
            SlackProfilePanelState::Loading { .. } => div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(slack_profile_title(title))
                .child(slack_profile_loading())
                .into_any_element(),
            SlackProfilePanelState::Error { message, .. } => div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(slack_profile_title(title))
                .child(slack_profile_error(message))
                .into_any_element(),
            SlackProfilePanelState::Loaded(profile) => {
                self.render_loaded_slack_profile(profile, cx)
            }
        }
    }

    /// Laid out like Slack's: the full name, pronouns and title, status,
    /// presence and local time, the Message button, then contact details.
    fn render_loaded_slack_profile(
        &self,
        profile: &SlackProfile,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let presence = self.slack_workspace().and_then(|workspace| {
            self.slack_presence_authority.resolve_presence(
                &workspace.team_id,
                &profile.user_id,
                None,
            )
        });
        let local_time = profile
            .timezone_id
            .as_deref()
            .and_then(slack_profile_local_time);
        let status = profile.status_text.clone().map(|text| {
            match profile
                .status_emoji
                .as_deref()
                .and_then(|code| emojis::get_by_shortcode(code.trim_matches(':')))
            {
                Some(emoji) => format!("{} {text}", emoji.as_str()),
                None => text,
            }
        });
        let subtitle = [profile.pronouns.clone(), profile.title.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        let user_id = profile.user_id.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(slack_profile_title(&profile.real_name))
                    .when(profile.display_name != profile.real_name, |this| {
                        this.child(slack_profile_secondary(&profile.display_name))
                    })
                    .when(!subtitle.is_empty(), |this| {
                        this.child(slack_profile_secondary(&subtitle))
                    }),
            )
            .when_some(status, |this, status| {
                this.child(slack_profile_line(div().child(status)))
            })
            .when_some(presence, |this, presence| {
                let (icon, label) = match presence {
                    SlackUserPresence::Active => (SlackShellIcon::PresenceActive, "Active"),
                    SlackUserPresence::Away => (SlackShellIcon::PresenceAway, "Away"),
                };
                this.child(slack_profile_line(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(slack_icon(icon, 0x1d9bd1, 14.0, cx))
                        .child(label),
                ))
            })
            .when_some(local_time, |this, time| {
                this.child(slack_profile_line(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .child(slack_icon(SlackShellIcon::Clock, 0xd1d2d3, 16.0, cx))
                        .child(format!("{time} local time")),
                ))
            })
            .child(slack_profile_message_button(user_id, cx))
            .when(profile.email.is_some() || profile.phone.is_some(), |this| {
                this.child(slack_profile_contacts(profile, cx))
            })
            .into_any_element()
    }
}

fn slack_profile_message_button(user_id: String, cx: &mut Context<SurfaceState>) -> Div {
    div().flex().gap(px(8.0)).child(
        div()
            .id("slack-profile-message")
            .h(px(36.0))
            .px(px(16.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(0xffffff, 0.25))
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.06)))
            .text_size(px(15.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_dm_with_user(&user_id, cx);
            }))
            .child(slack_icon(
                SlackShellIcon::MessageFilled,
                0xffffff,
                16.0,
                cx,
            ))
            .child("Message"),
    )
}

fn slack_profile_contacts(profile: &SlackProfile, cx: &mut Context<SurfaceState>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(div().h(px(1.0)).bg(alpha(0xffffff, 0.10)))
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("Contact information"),
        )
        .when_some(profile.email.clone(), |this, email| {
            this.child(slack_profile_contact(
                "✉️",
                "Email Address",
                email,
                true,
                cx,
            ))
        })
        .when_some(profile.phone.clone(), |this, phone| {
            this.child(slack_profile_contact("📞", "Phone", phone, false, cx))
        })
}

/// The time where the person is, like "11:01 PM".
pub(in crate::ui::surface) fn slack_profile_local_time(timezone_id: &str) -> Option<String> {
    let zone = timezone_id.parse::<chrono_tz::Tz>().ok()?;
    Some(
        chrono::Utc::now()
            .with_timezone(&zone)
            .format("%-I:%M %p")
            .to_string(),
    )
}

fn slack_profile_secondary(text: &str) -> Div {
    div()
        .text_size(px(15.0))
        .text_color(rgb(0xababad))
        .child(text.to_string())
}

fn slack_profile_line(content: Div) -> Div {
    content.text_size(px(15.0)).text_color(rgb(0xd1d2d3))
}

fn slack_profile_contact(
    icon: &'static str,
    label: &'static str,
    value: String,
    email: bool,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let link = email.then(|| format!("mailto:{value}"));
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .size(px(36.0))
                .flex_none()
                .rounded(px(8.0))
                .bg(alpha(0xffffff, 0.06))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .child(icon),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xd1d2d3))
                        .child(label),
                )
                .child({
                    let value_text = div()
                        .text_size(px(15.0))
                        .text_color(rgb(if link.is_some() { 0x1d9bd1 } else { 0xd1d2d3 }))
                        .child(value);
                    match link {
                        Some(link) => value_text
                            .id(label)
                            .cursor_pointer()
                            .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&link)))
                            .into_any_element(),
                        None => value_text.into_any_element(),
                    }
                }),
        )
}

fn slack_profile_avatar_label(panel: &SlackProfilePanelState, title: &str) -> String {
    match panel {
        SlackProfilePanelState::Loaded(profile) => profile
            .avatar_label
            .clone()
            .unwrap_or_else(|| initials(&profile.display_name)),
        _ => initials(title),
    }
}

fn slack_profile_avatar_image(panel: &SlackProfilePanelState) -> Option<&str> {
    match panel {
        SlackProfilePanelState::Loaded(profile) => profile.avatar_image_url.as_deref(),
        _ => None,
    }
}

fn slack_profile_avatar(
    avatar_label: &str,
    avatar_fill: u32,
    avatar_image: Option<Arc<Image>>,
) -> Div {
    if let Some(image) = avatar_image {
        return div()
            .size(px(SLACK_PROFILE_AVATAR_SIZE))
            .flex_none()
            .rounded(px(SLACK_PROFILE_AVATAR_RADIUS))
            .overflow_hidden()
            .child(
                img(image)
                    .w_full()
                    .h_full()
                    .rounded(px(SLACK_PROFILE_AVATAR_RADIUS)),
            );
    }

    div()
        .size(px(SLACK_PROFILE_AVATAR_SIZE))
        .flex_none()
        .rounded(px(SLACK_PROFILE_AVATAR_RADIUS))
        .bg(rgb(avatar_fill))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(72.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child(avatar_label.to_string())
}

fn slack_profile_title(title: &str) -> Div {
    div()
        .text_size(px(22.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child(title.to_string())
}

fn slack_profile_loading() -> Div {
    div()
        .text_size(px(13.0))
        .text_color(rgb(0x8d9299))
        .child("Loading profile...")
}

fn slack_profile_error(message: &str) -> Div {
    div()
        .text_size(px(13.0))
        .line_height(relative(1.4))
        .text_color(rgb(0xffb4b4))
        .child(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::slack_profile_local_time;

    #[test]
    fn local_time_reads_like_slack_and_needs_a_known_zone() {
        let time = slack_profile_local_time("Asia/Shanghai").unwrap();
        assert!(time.ends_with("AM") || time.ends_with("PM"), "{time}");
        assert_eq!(slack_profile_local_time("Not/AZone"), None);
    }
}
