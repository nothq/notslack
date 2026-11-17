use super::{
    alpha, div, img, px, rgb, slack_icon, slack_remote_image_cache_key, AnyElement, Arc, Context,
    Div, FluentBuilder, FontWeight, Image, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SlackMediaPlayback, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
#[cfg(test)]
use crate::ui::surface::SlackActivityDetailState;
use crate::ui::surface::{slack_base_icon_radius, slack_lightbox_shows_image};
use crate::ui::surface::{SlackAttachmentSelection, SlackMediaHostId};
use crate::ui::SlackAttachment;
use gpui::{ObjectFit, Role, StyledImage};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};
use viewer::{render_slack_lightbox_notice, slack_lightbox_ago};

mod lookup;
mod media_controls;
mod viewer;

/// The gap between the viewer and the window's edges.
const SLACK_LIGHTBOX_INSET: f32 = 32.0;
/// The bar of controls along the bottom of the viewer.
const SLACK_LIGHTBOX_TOOLBAR_HEIGHT: f32 = 68.0;

#[cfg(test)]
use lookup::find_slack_attachment_in_rows_by_title;

struct SlackMediaStatusRenderContext<'a> {
    selection: &'a SlackAttachmentSelection,
    label: &'static str,
    detail: Option<gpui::SharedString>,
    retry: bool,
}

impl SurfaceState {
    pub(crate) fn render_slack_attachment_lightbox(
        &self,
        selection: &SlackAttachmentSelection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let attachment = &selection.attachment;
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .top(px(0.0))
                .bottom(px(0.0))
                .bg(alpha(0x000000, 0.72)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_expanded_attachment(cx);
            }),
            cx,
        );
        div()
            .id("slack-attachment-lightbox")
            .role(Role::Dialog)
            .aria_label(format!("Preview {}", attachment.title))
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .top(px(0.0))
            .bottom(px(0.0))
            .child(backdrop)
            .child(self.render_slack_attachment_lightbox_card(selection, attachment, cx))
            .into_any_element()
    }

    fn render_slack_attachment_lightbox_card(
        &self,
        selection: &SlackAttachmentSelection,
        attachment: &SlackAttachment,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let thread_panel = self.slack_lightbox_thread_panel().cloned();
        let card = div()
            .id("slack-attachment-lightbox-card")
            .absolute()
            .left(px(SLACK_LIGHTBOX_INSET))
            .right(px(SLACK_LIGHTBOX_INSET))
            .top(px(SLACK_LIGHTBOX_INSET))
            .bottom(px(SLACK_LIGHTBOX_INSET))
            .rounded(px(10.0))
            .overflow_hidden()
            .border_1()
            .border_color(alpha(0xffffff, 0.10))
            .bg(rgb(0x1a1d21))
            .flex()
            .flex_col()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.close_slack_lightbox_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if event.pressed_button == Some(MouseButton::Left) {
                    this.drag_slack_lightbox(event.position, cx);
                } else {
                    this.end_slack_lightbox_drag();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| {
                    this.end_slack_lightbox_drag();
                    cx.notify();
                }),
            )
            .child(self.render_slack_attachment_lightbox_header(attachment, cx))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .flex()
                    .child(self.render_slack_attachment_lightbox_stage(selection, attachment, cx))
                    .when_some(thread_panel, |this, panel| {
                        this.child(self.render_slack_thread_panel(&panel, cx))
                    }),
            );
        match self.slack_lightbox_view() {
            Some(view) => card.track_focus(&view.focus).on_key_down(cx.listener(
                |this, event: &gpui::KeyDownEvent, window, cx| {
                    if this.handle_slack_lightbox_key(event, window, cx) {
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                },
            )),
            None => card,
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve_slack_attachment_selection_by_title(
        &self,
        attachment_title: &str,
    ) -> Option<SlackAttachmentSelection> {
        find_slack_attachment_in_rows_by_title(&self.slack_message_rows, attachment_title)
            .or_else(|| {
                let panel = self.slack_thread_panel.as_ref()?;
                find_slack_attachment_in_rows_by_title(
                    std::slice::from_ref(&panel.parent_row),
                    attachment_title,
                )
                .or_else(|| {
                    find_slack_attachment_in_rows_by_title(&panel.reply_rows, attachment_title)
                })
            })
            .or_else(|| {
                self.slack_all_threads_rows.iter().find_map(|thread| {
                    find_slack_attachment_in_rows_by_title(
                        std::slice::from_ref(&thread.parent),
                        attachment_title,
                    )
                    .or_else(|| {
                        find_slack_attachment_in_rows_by_title(&thread.replies, attachment_title)
                    })
                })
            })
            .or_else(|| {
                self.slack_later_rows
                    .iter()
                    .filter_map(|row| row.thread_target())
                    .find_map(|detail| {
                        find_slack_attachment_in_rows_by_title(
                            std::slice::from_ref(&detail.selected_message_row),
                            attachment_title,
                        )
                    })
            })
            .or_else(|| {
                self.slack_pins_rows.iter().find_map(|pin| {
                    find_slack_attachment_in_rows_by_title(
                        std::slice::from_ref(&pin.message),
                        attachment_title,
                    )
                })
            })
            .or_else(|| match &self.slack_activity_detail {
                SlackActivityDetailState::Loaded { rows, .. } => {
                    find_slack_attachment_in_rows_by_title(rows, attachment_title)
                }
                _ => None,
            })
            .map(SlackAttachmentSelection::from_row)
    }

    pub(crate) fn resolve_slack_attachment_selection_by_id(
        &self,
        attachment_id: &str,
    ) -> Option<SlackAttachmentSelection> {
        lookup::find_current_slack_attachment_selection_by_id(self, attachment_id)
    }

    fn render_slack_attachment_lightbox_header(
        &self,
        attachment: &SlackAttachment,
        cx: &mut Context<Self>,
    ) -> Div {
        let message = self.slack_lightbox_message();
        let author = message.map(|row| row.author.clone());
        let subtitle = self.slack_lightbox_subtitle(attachment);
        let avatar = message.map(|row| {
            let image = row
                .avatar_image_url
                .as_deref()
                .and_then(|url| self.slack_remote_images.get(url).cloned());
            let face = div()
                .size(px(36.0))
                .flex_none()
                .rounded(slack_base_icon_radius(36.0))
                .overflow_hidden();
            match image {
                Some(image) => face.child(img(image).size_full()),
                None => face
                    .bg(rgb(row.avatar_fill))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(row.avatar_text.clone()),
            }
        });
        div()
            .h(px(64.0))
            .flex_none()
            .px(px(16.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .border_b_1()
            .border_color(alpha(0xffffff, 0.08))
            .children(avatar)
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_grow(1.0)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .child(author.unwrap_or_else(|| attachment.title.clone())),
                    )
                    .when_some(subtitle, |this, subtitle| {
                        this.child(
                            div()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(13.0))
                                .text_color(rgb(0xc7cbd0))
                                .child(subtitle),
                        )
                    }),
            )
            .child(
                div()
                    .id("slack-attachment-lightbox-close")
                    .role(Role::Button)
                    .aria_label("Close attachment preview")
                    .focusable()
                    .tab_stop(true)
                    .size(px(36.0))
                    .flex_none()
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|style| style.bg(alpha(0xffffff, 0.08)))
                    .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_expanded_attachment(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space")
                            && !event.keystroke.modifiers.modified()
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.close_slack_expanded_attachment(cx);
                        }
                    }))
                    .child(slack_icon(SlackShellIcon::Close, 0xc8cbd0, 20.0, cx)),
            )
    }

    /// "4 hours ago in # ge-qb – image.png", like Slack's viewer.
    fn slack_lightbox_subtitle(&self, attachment: &SlackAttachment) -> Option<String> {
        let target = self.slack_lightbox_message()?.action_target.clone()?;
        let seconds = target
            .message_timestamp()
            .as_str()
            .split('.')
            .next()?
            .parse::<i64>()
            .ok()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(seconds);
        let mut subtitle = slack_lightbox_ago(now - seconds);
        let conversation = self
            .slack_sidebar_rows
            .iter()
            .find_map(|row| match &row.kind {
                crate::ui::surface::SlackSidebarRowKind::Item { item, .. }
                    if item.target_id == target.conversation_id() =>
                {
                    Some(item)
                }
                _ => None,
            });
        if let Some(item) = conversation {
            let prefix = if item.target_kind.is_channel() {
                "# "
            } else {
                ""
            };
            subtitle.push_str(&format!(" in {prefix}{}", item.label));
        }
        subtitle.push_str(&format!("  –  {}", attachment.title));
        Some(subtitle)
    }

    fn render_slack_attachment_lightbox_stage(
        &self,
        selection: &SlackAttachmentSelection,
        attachment: &SlackAttachment,
        cx: &mut Context<Self>,
    ) -> Div {
        let shows_image = slack_lightbox_shows_image(attachment)
            && self.slack_lightbox_image(attachment).is_some();
        let content = if shows_image {
            self.render_slack_lightbox_image(attachment, cx)
        } else {
            div()
                .size_full()
                .p(px(16.0))
                .pb(px(SLACK_LIGHTBOX_TOOLBAR_HEIGHT))
                .child(self.render_slack_attachment_lightbox_body(selection, attachment, cx))
                .into_any_element()
        };
        div()
            .relative()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .overflow_hidden()
            .bg(rgb(0x111214))
            .child(content)
            .child(self.render_slack_lightbox_toolbar(attachment, shows_image, cx))
            .when_some(
                self.slack_lightbox_view()
                    .and_then(|view| view.notice.clone()),
                |this, notice| this.child(render_slack_lightbox_notice(notice)),
            )
    }

    fn render_slack_attachment_lightbox_body(
        &self,
        selection: &SlackAttachmentSelection,
        attachment: &SlackAttachment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if attachment.media.is_some()
            && self
                .slack_workspace_api_capabilities
                .prepare_attachment_media
        {
            self.render_slack_attachment_media(selection, attachment, cx)
        } else if let Some(image) = self.slack_attachment_preview_image(attachment) {
            img(image)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else {
            div()
                .size_full()
                .rounded(px(12.0))
                .bg(rgb(0x1a1d21))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(15.0))
                .text_color(rgb(0xc7cbd0))
                .child(attachment.title.clone())
                .into_any_element()
        }
    }

    fn render_slack_attachment_media(
        &self,
        selection: &SlackAttachmentSelection,
        attachment: &SlackAttachment,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let media = attachment
            .media
            .as_ref()
            .expect("media lightbox must receive a typed Slack media attachment");
        let host = SlackMediaHostId::lightbox(selection.attachment_id.clone());
        let playback = self.slack_lightbox_media_playback(selection, media.file_id(), &host);
        if let Some(active_media) = slack_active_lightbox_media(playback) {
            return active_media;
        }
        match playback {
            Some(SlackMediaPlayback::Failed { error, .. }) => self
                .render_slack_media_status(
                    SlackMediaStatusRenderContext {
                        selection,
                        label: "Playback failed",
                        detail: Some(error.clone()),
                        retry: true,
                    },
                    cx,
                )
                .into_any_element(),
            Some(SlackMediaPlayback::Loading { .. }) => self
                .render_slack_media_status(
                    SlackMediaStatusRenderContext {
                        selection,
                        label: "Preparing playback…",
                        detail: None,
                        retry: false,
                    },
                    cx,
                )
                .into_any_element(),
            None => self
                .render_slack_media_status(
                    SlackMediaStatusRenderContext {
                        selection,
                        label: "Playback is not ready",
                        detail: None,
                        retry: true,
                    },
                    cx,
                )
                .into_any_element(),
            Some(SlackMediaPlayback::Video { .. } | SlackMediaPlayback::Audio { .. }) => {
                unreachable!("active Slack media playback should render before status handling")
            }
        }
    }

    fn slack_lightbox_media_playback<'a>(
        &'a self,
        selection: &SlackAttachmentSelection,
        file_id: &str,
        host: &SlackMediaHostId,
    ) -> Option<&'a SlackMediaPlayback> {
        self.slack_media_playback.as_ref().filter(|playback| {
            playback
                .target()
                .matches(selection.attachment_id.as_ref(), file_id)
                && playback.host() == host
        })
    }

    fn render_slack_media_status(
        &self,
        render: SlackMediaStatusRenderContext<'_>,
        cx: &mut Context<Self>,
    ) -> Div {
        let SlackMediaStatusRenderContext {
            selection,
            label,
            detail,
            retry,
        } = render;
        div()
            .size_full()
            .rounded(px(12.0))
            .bg(rgb(0x1a1d21))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .text_color(rgb(0xc7cbd0))
            .child(label)
            .when_some(detail, |this, detail| {
                this.child(
                    div()
                        .max_w(px(520.0))
                        .text_size(px(12.0))
                        .text_color(rgb(0x9ca2a9))
                        .child(detail),
                )
            })
            .when(retry, |this| {
                this.child(self.render_slack_media_retry_button(selection, cx))
            })
    }

    fn slack_attachment_preview_image(&self, attachment: &SlackAttachment) -> Option<Arc<Image>> {
        let key = slack_remote_image_cache_key(attachment);
        self.slack_attachment_preview_image_by_key(key.as_deref())
    }

    pub(crate) fn slack_attachment_preview_image_by_key(
        &self,
        preview_cache_key: Option<&str>,
    ) -> Option<Arc<Image>> {
        preview_cache_key.and_then(|key| self.slack_remote_images.get(key).cloned())
    }
}

fn slack_active_lightbox_media(playback: Option<&SlackMediaPlayback>) -> Option<AnyElement> {
    match playback? {
        SlackMediaPlayback::Video { player, .. } => Some(
            div()
                .size_full()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .rounded(px(10.0))
                .overflow_hidden()
                .child(player.clone())
                .into_any_element(),
        ),
        SlackMediaPlayback::Audio { player, .. } => Some(
            div()
                .size_full()
                .rounded(px(12.0))
                .bg(rgb(0x1a1d21))
                .flex()
                .items_center()
                .justify_center()
                .child(player.clone())
                .into_any_element(),
        ),
        SlackMediaPlayback::Loading { .. } | SlackMediaPlayback::Failed { .. } => None,
    }
}
