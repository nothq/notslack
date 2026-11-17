//! The image viewer inside the preview: the zoomable, turnable image and the
//! bar of controls under it, laid out like Slack's.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, px, rgb, AnyElement, AppContext, Context, CursorStyle, Div, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ObjectFit,
    ParentElement, Render, Role, ScrollWheelEvent, SharedString, Stateful,
    StatefulInteractiveElement, Styled, StyledImage, Window,
};

use crate::ui::surface::{
    alpha, slack_icon, SlackShellIcon, SurfaceState, SLACK_LIGHTBOX_MAX_ZOOM,
    SLACK_LIGHTBOX_MIN_ZOOM,
};
use crate::ui::SlackAttachment;

use super::SLACK_LIGHTBOX_TOOLBAR_HEIGHT;

const SLACK_LIGHTBOX_BUTTON_SIZE: f32 = 36.0;
const SLACK_LIGHTBOX_SLIDER_WIDTH: f32 = 96.0;
const SLACK_LIGHTBOX_SLIDER_KNOB: f32 = 14.0;
const SLACK_LIGHTBOX_ICON: u32 = 0xd1d2d3;
const SLACK_LIGHTBOX_BORDER: u32 = 0x565856;
/// The controls sit on a solid dark fill so they read over light images.
const SLACK_LIGHTBOX_CONTROL_BG: u32 = 0x1a1d21;

type SlackLightboxPress = Rc<dyn Fn(&mut SurfaceState, &mut Context<SurfaceState>)>;

/// One of the viewer's buttons.
struct SlackLightboxButton {
    id: &'static str,
    icon: SlackShellIcon,
    label: &'static str,
    detail: Option<&'static str>,
    bordered: bool,
    active: bool,
    press: SlackLightboxPress,
}

/// The dark label over a viewer control, like "Reset · Press space".
struct SlackLightboxTooltip {
    label: SharedString,
    detail: Option<&'static str>,
}

impl Render for SlackLightboxTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(6.0))
            .rounded(px(6.0))
            .bg(rgb(0x1d1c1d))
            .border_1()
            .border_color(alpha(0xffffff, 0.12))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(self.label.clone()),
            )
            .when_some(self.detail, |this, detail| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xababad))
                        .child(detail),
                )
            })
    }
}

impl SurfaceState {
    pub(super) fn render_slack_lightbox_image(
        &self,
        attachment: &SlackAttachment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some((image, _, _)) = self.slack_lightbox_image(attachment) else {
            return div().into_any_element();
        };
        let layout = self.slack_lightbox_layout(self.slack_lightbox_display_size());
        let view = self.slack_lightbox_view();
        let zoomed = view.is_some_and(|view| view.zoom > SLACK_LIGHTBOX_MIN_ZOOM);
        let dragging = view.is_some_and(|view| view.drag_from.is_some());
        // The image keeps its proportions while a turn is still being made.
        let picture = match layout {
            Some(layout) => img(image)
                .absolute()
                .left(layout.origin.x)
                .top(layout.origin.y)
                .w(layout.size.width)
                .h(layout.size.height)
                .object_fit(ObjectFit::Contain),
            None => img(image).size_full().object_fit(ObjectFit::Contain),
        };
        let surface = cx.entity().downgrade();
        div()
            .size_full()
            .on_children_prepainted(move |bounds, window, cx| {
                let Some(bounds) = bounds.first().copied() else {
                    return;
                };
                let changed = surface
                    .update(cx, |this, _| this.set_slack_lightbox_viewport(bounds))
                    .unwrap_or(false);
                if changed {
                    window.refresh();
                }
            })
            .child(
                div()
                    .id("slack-lightbox-viewport")
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .when(zoomed, |this| {
                        this.cursor(if dragging {
                            CursorStyle::ClosedHand
                        } else {
                            CursorStyle::OpenHand
                        })
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            if let Some(view) = this.slack_lightbox_view() {
                                window.focus(&view.focus, cx);
                            }
                            this.close_slack_lightbox_menu(cx);
                            this.begin_slack_lightbox_drag(event.position);
                            cx.notify();
                        }),
                    )
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        let delta = f32::from(event.delta.pixel_delta(px(20.0)).y);
                        let Some(zoom) = this.slack_lightbox_view().map(|view| view.zoom) else {
                            return;
                        };
                        if delta != 0.0 {
                            this.set_slack_lightbox_zoom(zoom + delta / 400.0, cx);
                        }
                    }))
                    .child(picture),
            )
            .into_any_element()
    }

    pub(super) fn render_slack_lightbox_toolbar(
        &self,
        attachment: &SlackAttachment,
        shows_image: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let view = self.slack_lightbox_view();
        let adjusted = view.is_some_and(|view| view.is_adjusted());
        let menu_open = view.is_some_and(|view| view.menu_open);
        let has_message = self
            .slack_lightbox_message()
            .is_some_and(|row| row.action_target.is_some());
        let has_link = !attachment.link_url.is_empty();
        let thread_shown = self.slack_lightbox_thread_panel().is_some();
        let left = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(shows_image, |this| {
                this.child(self.render_slack_lightbox_button(
                    SlackLightboxButton {
                        id: "slack-lightbox-rotate",
                        icon: SlackShellIcon::ViewerRotate,
                        label: "Rotate",
                        detail: None,
                        bordered: true,
                        active: false,
                        press: Rc::new(|this, cx| this.rotate_slack_lightbox(cx)),
                    },
                    cx,
                ))
                .child(self.render_slack_lightbox_zoom(cx))
                .when(adjusted, |this| {
                    this.child(self.render_slack_lightbox_button(
                        SlackLightboxButton {
                            id: "slack-lightbox-reset",
                            icon: SlackShellIcon::ViewerReset,
                            label: "Reset",
                            detail: Some("Press space"),
                            bordered: true,
                            active: false,
                            press: Rc::new(|this, cx| this.reset_slack_lightbox(cx)),
                        },
                        cx,
                    ))
                })
            });
        let right = div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .when(shows_image, |this| {
                this.child(self.render_slack_lightbox_button(
                    SlackLightboxButton {
                        id: "slack-lightbox-download",
                        icon: SlackShellIcon::ViewerDownload,
                        label: "Download",
                        detail: None,
                        bordered: false,
                        active: false,
                        press: Rc::new(|this, cx| this.download_slack_lightbox_image(cx)),
                    },
                    cx,
                ))
            })
            .when(has_message, |this| {
                this.child(self.render_slack_lightbox_button(
                    SlackLightboxButton {
                        id: "slack-lightbox-thread",
                        icon: SlackShellIcon::ViewerThread,
                        label: if thread_shown {
                            "Hide thread"
                        } else {
                            "Show thread"
                        },
                        detail: None,
                        bordered: false,
                        active: thread_shown,
                        press: Rc::new(|this, cx| this.toggle_slack_lightbox_thread(cx)),
                    },
                    cx,
                ))
            })
            .when(has_message || has_link, |this| {
                this.child(self.render_slack_lightbox_button(
                    SlackLightboxButton {
                        id: "slack-lightbox-more",
                        icon: SlackShellIcon::More,
                        label: "More actions",
                        detail: None,
                        bordered: false,
                        active: menu_open,
                        press: Rc::new(|this, cx| this.toggle_slack_lightbox_menu(cx)),
                    },
                    cx,
                ))
            });
        div()
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .h(px(SLACK_LIGHTBOX_TOOLBAR_HEIGHT))
            .px(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .bg(gpui::linear_gradient(
                0.0,
                gpui::linear_color_stop(alpha(0x111214, 0.92), 0.0),
                gpui::linear_color_stop(alpha(0x111214, 0.0), 1.0),
            ))
            .child(left)
            .child(right)
            .when(menu_open, |this| {
                this.child(self.render_slack_lightbox_menu(has_message, has_link, cx))
            })
    }

    fn render_slack_lightbox_button(
        &self,
        button: SlackLightboxButton,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let SlackLightboxButton {
            id,
            icon,
            label,
            detail,
            bordered,
            active,
            press,
        } = button;
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .size(px(SLACK_LIGHTBOX_BUTTON_SIZE))
            .flex_none()
            .rounded(px(8.0))
            .when(bordered, |this| {
                this.border_1().border_color(rgb(SLACK_LIGHTBOX_BORDER))
            })
            .bg(if active {
                rgb(0x3b3d42)
            } else {
                rgb(SLACK_LIGHTBOX_CONTROL_BG)
            })
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(rgb(0x35373b)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| press(this, cx)))
            .tooltip(move |_, cx| {
                cx.new(|_| SlackLightboxTooltip {
                    label: label.into(),
                    detail,
                })
                .into()
            })
            .child(slack_icon(icon, SLACK_LIGHTBOX_ICON, 20.0, cx))
    }

    /// The − slider + group; the slider runs from fitted to twice that.
    fn render_slack_lightbox_zoom(&self, cx: &mut Context<Self>) -> Div {
        let view = self.slack_lightbox_view();
        let zoom = view.map_or(SLACK_LIGHTBOX_MIN_ZOOM, |view| view.zoom);
        let percent = view.map_or(100, |view| view.zoom_percent());
        let fraction =
            (zoom - SLACK_LIGHTBOX_MIN_ZOOM) / (SLACK_LIGHTBOX_MAX_ZOOM - SLACK_LIGHTBOX_MIN_ZOOM);
        let knob_left = fraction * (SLACK_LIGHTBOX_SLIDER_WIDTH - SLACK_LIGHTBOX_SLIDER_KNOB);
        let surface = cx.entity().downgrade();
        let slider = div()
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first().copied() {
                    surface
                        .update(cx, |this, _| this.set_slack_lightbox_slider(bounds))
                        .ok();
                }
            })
            .child(
                div()
                    .id("slack-lightbox-zoom-slider")
                    .role(Role::Slider)
                    .aria_label(format!("Zoom {percent}%"))
                    .relative()
                    .w(px(SLACK_LIGHTBOX_SLIDER_WIDTH))
                    .h(px(SLACK_LIGHTBOX_BUTTON_SIZE))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.set_slack_lightbox_slider_dragging(true);
                            this.set_slack_lightbox_zoom_at(event.position.x, cx);
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(SLACK_LIGHTBOX_SLIDER_KNOB / 2.0))
                            .right(px(SLACK_LIGHTBOX_SLIDER_KNOB / 2.0))
                            .top(px(SLACK_LIGHTBOX_BUTTON_SIZE / 2.0 - 1.0))
                            .h(px(2.0))
                            .rounded(px(1.0))
                            .bg(rgb(0xababad)),
                    )
                    .child(
                        div()
                            .id("slack-lightbox-zoom-knob")
                            .absolute()
                            .left(px(knob_left))
                            .top(px((SLACK_LIGHTBOX_BUTTON_SIZE
                                - SLACK_LIGHTBOX_SLIDER_KNOB)
                                / 2.0))
                            .size(px(SLACK_LIGHTBOX_SLIDER_KNOB))
                            .rounded_full()
                            .bg(rgb(0x1d9bd1))
                            .tooltip(move |_, cx| {
                                cx.new(|_| SlackLightboxTooltip {
                                    label: format!("{percent}%").into(),
                                    detail: None,
                                })
                                .into()
                            }),
                    ),
            );
        div()
            .h(px(SLACK_LIGHTBOX_BUTTON_SIZE))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(SLACK_LIGHTBOX_BORDER))
            .bg(rgb(SLACK_LIGHTBOX_CONTROL_BG))
            .flex()
            .items_center()
            .child(self.render_slack_lightbox_zoom_step("slack-lightbox-zoom-out", -1.0, cx))
            .child(slider)
            .child(self.render_slack_lightbox_zoom_step("slack-lightbox-zoom-in", 1.0, cx))
    }

    fn render_slack_lightbox_zoom_step(
        &self,
        id: &'static str,
        steps: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (icon, label) = if steps > 0.0 {
            (SlackShellIcon::ViewerZoomIn, "Zoom in")
        } else {
            (SlackShellIcon::ViewerZoomOut, "Zoom out")
        };
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .w(px(32.0))
            .h_full()
            .rounded(px(7.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(rgb(0x35373b)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.step_slack_lightbox_zoom(steps, cx);
            }))
            .child(slack_icon(icon, SLACK_LIGHTBOX_ICON, 18.0, cx))
    }

    fn render_slack_lightbox_menu(
        &self,
        has_message: bool,
        has_link: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let saved = self
            .slack_lightbox_message()
            .is_some_and(|row| row.saved_state.is_some());
        let row = |id: &'static str, label: &'static str, press: SlackLightboxPress| {
            div()
                .id(id)
                .role(Role::MenuItem)
                .aria_label(label)
                .h(px(32.0))
                .px(px(20.0))
                .flex()
                .items_center()
                .text_size(px(15.0))
                .text_color(rgb(0xd1d2d3))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
                .on_click(cx.listener(move |this, _, _, cx| press(this, cx)))
                .child(label)
        };
        let divider = || div().my(px(8.0)).h(px(1.0)).bg(alpha(0xffffff, 0.12));
        div()
            .id("slack-lightbox-menu")
            .role(Role::Menu)
            .absolute()
            .right(px(12.0))
            .bottom(px(SLACK_LIGHTBOX_TOOLBAR_HEIGHT - 8.0))
            .w(px(280.0))
            .py(px(10.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(0xffffff, 0.12))
            .bg(rgb(0x222529))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(has_message, |this| {
                this.child(row(
                    "slack-lightbox-menu-thread",
                    "View thread",
                    Rc::new(|this, cx| {
                        this.close_slack_lightbox_menu(cx);
                        if this.slack_lightbox_thread_panel().is_none() {
                            this.toggle_slack_lightbox_thread(cx);
                        }
                    }),
                ))
                .child(row(
                    "slack-lightbox-menu-saved",
                    if saved {
                        "Remove from saved"
                    } else {
                        "Add to saved"
                    },
                    Rc::new(|this, cx| this.toggle_slack_lightbox_message_saved(cx)),
                ))
            })
            .when(has_message && has_link, |this| this.child(divider()))
            .when(has_link, |this| {
                this.child(row(
                    "slack-lightbox-menu-copy-link",
                    "Copy link to file",
                    Rc::new(|this, cx| this.copy_slack_lightbox_file_link(cx)),
                ))
            })
    }

    /// Space resets, + and − zoom, while the viewer itself has focus (not
    /// the thread's reply box beside it).
    pub(super) fn handle_slack_lightbox_key(
        &mut self,
        event: &KeyDownEvent,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let focused = self
            .slack_lightbox_view()
            .is_some_and(|view| view.focus.is_focused(window));
        let modifiers = event.keystroke.modifiers;
        if !focused || modifiers.control || modifiers.alt || modifiers.platform {
            return false;
        }
        match event.keystroke.key.as_str() {
            "space" => self.reset_slack_lightbox(cx),
            "=" | "+" => self.step_slack_lightbox_zoom(1.0, cx),
            "-" => self.step_slack_lightbox_zoom(-1.0, cx),
            _ => return false,
        }
        true
    }
}

pub(super) fn render_slack_lightbox_notice(notice: SharedString) -> Div {
    div()
        .absolute()
        .top(px(16.0))
        .left(px(0.0))
        .right(px(0.0))
        .flex()
        .justify_center()
        .child(
            div()
                .px(px(14.0))
                .py(px(8.0))
                .rounded(px(8.0))
                .bg(rgb(0x1d1c1d))
                .border_1()
                .border_color(alpha(0xffffff, 0.12))
                .text_size(px(13.0))
                .text_color(rgb(0xffffff))
                .child(notice),
        )
}

/// "just now", "4 hours ago", "3 days ago".
pub(super) fn slack_lightbox_ago(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let (amount, unit) = match seconds {
        0..=59 => return "just now".to_string(),
        60..=3_599 => (seconds / 60, "minute"),
        3_600..=86_399 => (seconds / 3_600, "hour"),
        86_400..=2_591_999 => (seconds / 86_400, "day"),
        2_592_000..=31_535_999 => (seconds / 2_592_000, "month"),
        _ => (seconds / 31_536_000, "year"),
    };
    let plural = if amount == 1 { "" } else { "s" };
    format!("{amount} {unit}{plural} ago")
}

#[cfg(test)]
mod tests {
    use super::slack_lightbox_ago;

    #[test]
    fn ages_read_like_slack() {
        assert_eq!(slack_lightbox_ago(10), "just now");
        assert_eq!(slack_lightbox_ago(60), "1 minute ago");
        assert_eq!(slack_lightbox_ago(4 * 3_600 + 5), "4 hours ago");
        assert_eq!(slack_lightbox_ago(3 * 86_400), "3 days ago");
    }
}
