use std::sync::Arc;

use super::{
    alpha, div, img, px, relative, rgb, slack_palette, AnyElement, Context, Div, FontWeight, Image,
    InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    SurfaceState,
};
use crate::ui::surface::{SlackAppDmLoad, SlackAppDmTab, SlackPalette};
use crate::ui::{SlackAppDmDetails, SlackWorkspace};
use gpui::{Role, SharedString};

const SLACK_APP_ABOUT_ICON_SIZE: f32 = 72.0;
const SLACK_APP_ABOUT_MAX_WIDTH: f32 = 720.0;

impl SurfaceState {
    pub(super) fn render_slack_app_dm_panel(
        &self,
        workspace: &SlackWorkspace,
        tab: SlackAppDmTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = match tab {
            SlackAppDmTab::Home => self.render_slack_app_home(cx),
            SlackAppDmTab::About | SlackAppDmTab::Messages => {
                self.render_slack_app_about(workspace, cx)
            }
        };
        div()
            .id("slack-app-dm-panel")
            .role(Role::TabPanel)
            .aria_label(match tab {
                SlackAppDmTab::Home => "App home",
                SlackAppDmTab::About | SlackAppDmTab::Messages => "About this app",
            })
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_y_scroll()
            .px(px(20.0))
            .py(px(20.0))
            .child(content)
            .into_any_element()
    }

    fn render_slack_app_home(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let details = self.slack_app_dm.current_details();
        let Some(key) = self.current_slack_app_home_view_key() else {
            return match details {
                Some(SlackAppDmLoad::Loading) | None => slack_app_dm_muted("Loading…", &palette),
                Some(SlackAppDmLoad::Failed(error)) => {
                    slack_app_dm_error("app details", error.clone(), &palette)
                }
                Some(SlackAppDmLoad::Loaded(_)) => {
                    slack_app_dm_muted("This app has no Home tab content yet.", &palette)
                }
            };
        };
        match self.slack_app_dm.home_views.get(&key) {
            Some(SlackAppDmLoad::Loading) | None => slack_app_dm_muted("Loading…", &palette),
            Some(SlackAppDmLoad::Failed(error)) => {
                slack_app_dm_error("the Home tab", error.clone(), &palette)
            }
            Some(SlackAppDmLoad::Loaded(None)) => {
                slack_app_dm_muted("This app has no Home tab content yet.", &palette)
            }
            Some(SlackAppDmLoad::Loaded(Some(body))) => div()
                .w_full()
                .min_w(px(0.0))
                .child(self.render_slack_standalone_message_body(body, cx)),
        }
    }

    fn render_slack_app_about(&self, workspace: &SlackWorkspace, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let dm_app = workspace.dm_app.as_ref();
        let details = self.slack_app_dm.current_details();
        let loaded: Option<&Arc<SlackAppDmDetails>> = details.and_then(SlackAppDmLoad::loaded);
        let profile = loaded.and_then(|details| details.profile.as_ref());
        let name: SharedString = profile
            .and_then(|profile| profile.name.clone())
            .or_else(|| dm_app.map(|app| app.name.clone()))
            .unwrap_or_else(|| workspace.channel_name.clone())
            .into();
        let icon_url = profile
            .and_then(|profile| profile.icon_url.as_deref())
            .or_else(|| dm_app.and_then(|app| app.avatar_image_url.as_deref()));
        let icon = icon_url.and_then(|url| self.slack_remote_images.get(url).cloned());
        let error = match details {
            Some(SlackAppDmLoad::Failed(error)) => Some(error.clone()),
            Some(SlackAppDmLoad::Loaded(details)) if details.profile.is_none() => details
                .error
                .clone()
                .map(SharedString::from)
                .or_else(|| Some("App details are unavailable.".into())),
            _ => None,
        };
        let loading = matches!(details, Some(SlackAppDmLoad::Loading) | None);

        let mut column = div()
            .w_full()
            .max_w(px(SLACK_APP_ABOUT_MAX_WIDTH))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(slack_app_about_icon(icon, &name, &palette))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.main_text))
                            .child(name.clone()),
                    )
                    .child(slack_app_badge(&palette)),
            );
        if let Some(desc) = profile.and_then(|profile| profile.desc.clone()) {
            column = column.child(
                div()
                    .text_size(px(15.0))
                    .line_height(relative(1.45))
                    .text_color(rgb(palette.main_text))
                    .child(desc),
            );
        }
        if let Some(long_desc) = profile.and_then(|profile| profile.long_desc.as_deref()) {
            column = column.child(slack_app_long_description(long_desc, &palette));
        }
        if let Some(developer) = profile.and_then(|profile| profile.developer_name.clone()) {
            column = column.child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child(format!("Developer: {developer}")),
            );
        }
        if let Some(url) = profile.and_then(|profile| profile.url.clone()) {
            column = column.child(slack_app_url_link(url, &palette, cx));
        }
        if loading {
            column = column.child(slack_app_dm_muted("Loading…", &palette));
        }
        if let Some(error) = error {
            column = column.child(slack_app_dm_error("app details", error, &palette));
        }
        column
    }
}

fn slack_app_long_description(long_desc: &str, palette: &SlackPalette) -> Div {
    div()
        .flex()
        .flex_col()
        .text_size(px(15.0))
        .line_height(relative(1.45))
        .text_color(rgb(palette.main_text))
        .children(long_desc.split('\n').map(|line| {
            if line.trim().is_empty() {
                div().h(px(10.0))
            } else {
                div().child(line.to_string())
            }
        }))
}

fn slack_app_url_link(
    url: String,
    palette: &SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> impl IntoElement {
    let target = url.clone();
    div()
        .id("slack-app-about-url")
        .role(Role::Link)
        .aria_label(SharedString::from(url.clone()))
        .text_size(px(13.0))
        .text_color(rgb(palette.link))
        .cursor_pointer()
        .hover(|style| style.underline())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.open_slack_link(&target, cx);
        }))
        .child(url)
}

fn slack_app_about_icon(icon: Option<Arc<Image>>, name: &str, palette: &SlackPalette) -> Div {
    let frame = div()
        .size(px(SLACK_APP_ABOUT_ICON_SIZE))
        .flex_none()
        .rounded(px(12.0))
        .overflow_hidden();
    match icon {
        Some(icon) => frame.child(img(icon).size_full().rounded(px(12.0))),
        None => frame
            .bg(alpha(palette.main_muted_text, 0.18))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(30.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(palette.main_text))
            .child(
                name.chars()
                    .find(|character| character.is_alphanumeric())
                    .map(|character| character.to_uppercase().to_string())
                    .unwrap_or_default(),
            ),
    }
}

fn slack_app_badge(palette: &SlackPalette) -> Div {
    div()
        .flex_none()
        .px(px(4.0))
        .rounded(px(3.0))
        .bg(alpha(palette.main_muted_text, 0.18))
        .text_size(px(10.0))
        .line_height(px(14.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(palette.main_secondary_text))
        .child("APP")
}

fn slack_app_dm_muted(text: &'static str, palette: &SlackPalette) -> Div {
    div()
        .text_size(px(14.0))
        .text_color(rgb(palette.main_muted_text))
        .child(text)
}

fn slack_app_dm_error(what: &str, error: SharedString, palette: &SlackPalette) -> Div {
    div()
        .text_size(px(13.0))
        .line_height(relative(1.4))
        .text_color(rgb(palette.main_muted_text))
        .child(SharedString::from(format!("Couldn't load {what}: {error}")))
}
