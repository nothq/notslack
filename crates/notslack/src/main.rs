use std::{borrow::Cow, sync::Arc};

use app_model::{AppearanceMode, SurfaceFrame, SurfaceRoot as _, SurfaceTheme, Viewport};
use chat::{live, ui};
use gpui::{
    actions, div, point, prelude::*, px, rgb, size, App, Bounds, Context, FocusHandle, KeyBinding,
    KeyDownEvent, Menu, MenuItem, SharedString, TitlebarOptions, Window, WindowAppearance,
    WindowBounds, WindowOptions,
};

actions!(notslack, [Quit]);

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../assets/fonts/Lato-Regular.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/Lato-Bold.ttf").as_slice()),
                Cow::Borrowed(include_bytes!("../assets/fonts/Lato-Black.ttf").as_slice()),
            ])
            .expect("bundled Lato fonts load");
        theme::init(theme::LoadThemes::JustBase, cx);
        cx.set_global(appearance_mode(cx.window_appearance()));
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus([Menu::new("notslack").items([MenuItem::action("Quit notslack", Quit)])]);
        cx.on_window_closed(|cx, _| cx.quit()).detach();

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1280.0), px(860.0)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("notslack".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.0), px(14.0))),
            }),
            window_min_size: Some(size(px(854.0), px(480.0))),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| cx.new(|cx| Notslack::new(window, cx)))
            .expect("open the notslack window");
        cx.activate(true);
    });
}

enum Content {
    Chat(Box<ui::SurfaceRoot>),
    Disconnected(SharedString),
}

struct Notslack {
    content: Content,
    focus: FocusHandle,
}

impl Notslack {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        window
            .observe_window_appearance(|window, cx| {
                cx.set_global(appearance_mode(window.appearance()));
            })
            .detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            content: connect(),
            focus,
        }
    }

    fn reconnect(&mut self, cx: &mut Context<Self>) {
        self.content = Content::Disconnected("Connecting to Slack…".into());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let captured = cx
                .background_spawn(async { live::capture_slack_desktop_credentials() })
                .await;
            this.update(cx, |this, cx| {
                this.content = match captured {
                    Ok(()) => connect(),
                    Err(error) => Content::Disconnected(error.into()),
                };
                cx.notify();
            })
        })
        .detach();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Content::Chat(root) = &mut self.content {
            if root.allows_workspace_shell_key_down(window, cx)
                && root.handle_workspace_key_down(event, cx)
            {
                cx.stop_propagation();
            }
        }
    }
}

fn connect() -> Content {
    let host = match live::SlackHostRuntime::load_for_desktop_app_discarding_events() {
        Ok(host) => Arc::new(host),
        Err(error) => return Content::Disconnected(error.into()),
    };
    if !host.is_available() {
        let reason = match host.integration_status() {
            live::SlackDesktopIntegrationStatus::Unavailable(reason) => reason.to_string(),
            live::SlackDesktopIntegrationStatus::Available => {
                live::SlackDesktopIntegrationUnavailable::NoWorkspace.to_string()
            }
        };
        return Content::Disconnected(reason.into());
    }
    Content::Chat(Box::new(ui::SurfaceRoot::production(
        live::production_slack_connection_api(None, host),
        None,
        media_capture::production_media_capture_service(),
        live::production_slack_local_file_api(),
    )))
}

fn appearance_mode(appearance: WindowAppearance) -> AppearanceMode {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => AppearanceMode::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => AppearanceMode::Dark,
    }
}

impl Render for Notslack {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = window.bounds();
        let viewport = Viewport {
            logical_width: f32::from(bounds.size.width).round() as u32,
            logical_height: f32::from(bounds.size.height).round() as u32,
            scale_factor: f64::from(window.scale_factor()),
            ..Viewport::default()
        };
        let theme = SurfaceTheme::for_appearance_mode(*cx.global::<AppearanceMode>());
        let root = div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .bg(rgb(theme.app_bg))
            .font_family("Lato");
        match &mut self.content {
            Content::Chat(chat) => {
                let frame = SurfaceFrame {
                    theme,
                    viewport,
                    preview_width: viewport.app_width(),
                    viewport_height: viewport.app_height(),
                    active: true,
                    window_controls_visible: true,
                    chrome_top_inset: 0.0,
                    ..SurfaceFrame::default()
                };
                chat.prepare_surface_window_frame(frame, window, cx);
                root.child(chat.render_surface(window, cx))
            }
            Content::Disconnected(reason) => root
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .text_color(rgb(theme.text_primary))
                .child(div().text_xl().font_weight(gpui::FontWeight::BOLD).child("notslack"))
                .child(div().text_color(rgb(theme.text_secondary)).child(reason.clone()))
                .child(
                    div()
                        .text_color(rgb(theme.text_muted))
                        .child(live::SLACK_DESKTOP_CONNECTION_DISCLOSURE),
                )
                .child(
                    div()
                        .id("connect")
                        .px_4()
                        .py_2()
                        .rounded_md()
                        .bg(rgb(0x007a5a))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .child("Connect Slack")
                        .on_click(cx.listener(|this, _, _, cx| this.reconnect(cx))),
                ),
        }
    }
}
