use super::{
    alpha, div, img, px, relative, rgb, AnyElement, Arc, Context, Div, FluentBuilder, FontWeight,
    Image, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackProfilePanelState, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_PROFILE_PANEL_WIDTH,
};
use crate::ui::initials;
use crate::ui::slack_avatar_fill;
use crate::ui::SlackProfile;

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
                    .child(self.render_slack_profile_body(panel, body)))
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
                    .text_size(px(15.0))
                    .font_weight(FontWeight::MEDIUM)
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
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(0xc8cbd0))
                    .child("x"),
            )
    }

    fn render_slack_profile_body(
        &self,
        panel: &SlackProfilePanelState,
        body: SlackProfileBodyConfig,
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
            .child(slack_profile_title(&body.title))
            .child(self.render_slack_profile_state(panel))
            .into_any_element()
    }

    fn render_slack_profile_state(&self, panel: &SlackProfilePanelState) -> AnyElement {
        match panel {
            SlackProfilePanelState::Loading { .. } => slack_profile_loading().into_any_element(),
            SlackProfilePanelState::Error { message, .. } => {
                slack_profile_error(message).into_any_element()
            }
            SlackProfilePanelState::Loaded(profile) => self.render_loaded_slack_profile(profile),
        }
    }

    fn render_loaded_slack_profile(&self, profile: &SlackProfile) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(0xb8bcc2))
                    .child(profile.real_name.clone()),
            )
            .when_some(profile.title.clone(), |this, field| {
                this.child(self.render_slack_profile_field("Title", &field))
            })
            .when_some(profile.status_text.clone(), |this, field| {
                this.child(self.render_slack_profile_field("Status", &field))
            })
            .when_some(profile.email.clone(), |this, field| {
                this.child(self.render_slack_profile_field("Email", &field))
            })
            .when_some(profile.phone.clone(), |this, field| {
                this.child(self.render_slack_profile_field("Phone", &field))
            })
            .when_some(profile.timezone_label.clone(), |this, field| {
                this.child(self.render_slack_profile_field("Timezone", &field))
            })
            .into_any_element()
    }

    fn render_slack_profile_field(&self, label: &str, value: &str) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(3.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0x8d9299))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .line_height(relative(1.4))
                    .text_color(rgb(0xffffff))
                    .child(value.to_string()),
            )
    }
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
