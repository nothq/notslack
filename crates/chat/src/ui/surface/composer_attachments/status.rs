use gpui::{Role, Stateful};

use super::SlackComposerAttachmentOwner;
use crate::ui::surface::{
    div, px, rgb, slack_icon, slack_palette, slack_spinner, AnyElement, Context, Div,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SlackComposerAttachmentKind,
    SlackComposerAttachmentPresentation, SlackComposerAttachmentStatus, SlackComposerFileId,
    SlackPalette, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};

pub(super) struct SlackComposerAttachmentStatusContext<'a> {
    owner: SlackComposerAttachmentOwner,
    context_id: &'a str,
    card_hover_group: &'a str,
}

impl<'a> SlackComposerAttachmentStatusContext<'a> {
    pub(super) fn new(
        owner: SlackComposerAttachmentOwner,
        context_id: &'a str,
        card_hover_group: &'a str,
    ) -> Self {
        Self {
            owner,
            context_id,
            card_hover_group,
        }
    }
}

impl SurfaceState {
    pub(super) fn render_slack_composer_attachment_status_button(
        &self,
        presentation: &SlackComposerAttachmentPresentation,
        context: SlackComposerAttachmentStatusContext<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if presentation.status == SlackComposerAttachmentStatus::Ready {
            self.render_slack_ready_attachment_remove_button(presentation, context, cx)
        } else {
            self.render_slack_pending_attachment_status_button(presentation, context, cx)
        }
    }

    fn render_slack_ready_attachment_remove_button(
        &self,
        presentation: &SlackComposerAttachmentPresentation,
        context: SlackComposerAttachmentStatusContext<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let file_id = presentation.file_id;
        let button_id = format!(
            "slack-composer-attachment-remove-{}-{file_id}",
            context.context_id
        );
        let button_hover_group = format!(
            "slack-composer-attachment-status-{}-{file_id}",
            context.context_id
        );
        let owner = context.owner;
        let button = div()
            .id(button_id)
            .group(button_hover_group.clone())
            .role(Role::Button)
            .aria_label(slack_composer_attachment_remove_label(presentation.kind))
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .absolute()
            .top(px(-7.0))
            .right(px(-7.0))
            .size(px(20.0))
            .rounded_full()
            .opacity(0.0)
            .group_hover(context.card_hover_group.to_string(), |style| {
                style.opacity(1.0)
            })
            .focus_visible(|style| {
                style
                    .opacity(1.0)
                    .border_2()
                    .border_color(rgb(palette.link))
            });
        self.bind_slack_attachment_remove_action(button, owner, file_id, cx)
            .child(slack_composer_close_icon_layers(
                &button_hover_group,
                palette.main_secondary_text,
                palette.main_text,
                palette.main_bg,
                cx,
            ))
            .into_any_element()
    }

    fn render_slack_pending_attachment_status_button(
        &self,
        presentation: &SlackComposerAttachmentPresentation,
        context: SlackComposerAttachmentStatusContext<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let file_id = presentation.file_id;
        let button_id = format!(
            "slack-composer-attachment-remove-{}-{file_id}",
            context.context_id
        );
        let button_hover_group = format!(
            "slack-composer-attachment-status-{}-{file_id}",
            context.context_id
        );
        let status = slack_composer_attachment_status(
            &self.slack_spinner_frame_cache,
            presentation.status,
            file_id,
            palette,
            cx,
        );
        let owner = context.owner;
        let button = div()
            .id(button_id.clone())
            .role(Role::Button)
            .aria_label(slack_composer_attachment_remove_label(presentation.kind))
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .absolute()
            .size_full()
            .rounded_full()
            .bg(rgb(palette.main_bg))
            .opacity(1.0)
            .group_hover(button_hover_group.clone(), |style| style.opacity(0.0))
            .focus_visible(|style| style.opacity(0.0));
        let button = self
            .bind_slack_attachment_remove_action(button, owner, file_id, cx)
            .flex()
            .items_center()
            .justify_center()
            .child(status);
        div()
            .id(format!("{button_id}-container"))
            .group(button_hover_group.clone())
            .absolute()
            .top(px(-10.0))
            .right(px(-10.0))
            .size(px(22.0))
            .child(slack_composer_attachment_close_layer(
                &button_hover_group,
                palette,
                cx,
            ))
            .child(button)
            .into_any_element()
    }

    fn bind_slack_attachment_remove_action(
        &self,
        button: Stateful<Div>,
        owner: SlackComposerAttachmentOwner,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let keyboard_owner = owner.clone();
        button
            .on_click(cx.listener(move |this, _, _, cx| {
                this.remove_slack_composer_attachment(&owner, file_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !slack_composer_attachment_action_key(event) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.remove_slack_composer_attachment(&keyboard_owner, file_id, cx);
            }))
    }

    pub(super) fn remove_slack_composer_attachment(
        &mut self,
        owner: &SlackComposerAttachmentOwner,
        file_id: SlackComposerFileId,
        cx: &mut Context<Self>,
    ) {
        match owner {
            SlackComposerAttachmentOwner::Main(handle) => {
                self.remove_slack_main_draft_attachment(handle, file_id, cx);
            }
            SlackComposerAttachmentOwner::Thread(handle) => {
                self.remove_slack_thread_reply_attachment_for_handle(handle, file_id, cx);
            }
        }
    }
}

fn slack_composer_attachment_remove_label(kind: SlackComposerAttachmentKind) -> &'static str {
    match kind {
        SlackComposerAttachmentKind::Audio => "Remove audio clip",
        SlackComposerAttachmentKind::Video => "Remove video clip",
        SlackComposerAttachmentKind::Image
        | SlackComposerAttachmentKind::Pdf
        | SlackComposerAttachmentKind::Text
        | SlackComposerAttachmentKind::File => "Remove file",
    }
}

fn slack_composer_attachment_status(
    spinner_frame_cache: &crate::ui::surface::SlackSpinnerFrameCache,
    status: SlackComposerAttachmentStatus,
    file_id: SlackComposerFileId,
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    match status {
        SlackComposerAttachmentStatus::Loading => slack_spinner(
            spinner_frame_cache,
            format!("slack-composer-attachment-spinner-{file_id}"),
            palette.main_secondary_text,
            18.0,
            cx,
        ),
        SlackComposerAttachmentStatus::Error => {
            slack_icon(SlackShellIcon::WarningFilled, 0xe01e5a, 20.0, cx)
        }
        SlackComposerAttachmentStatus::Ready => unreachable!(),
    }
}

fn slack_composer_attachment_close_layer(
    hover_group: &str,
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .absolute()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(slack_composer_close_icon_layers(
            hover_group,
            palette.main_secondary_text,
            palette.main_text,
            palette.main_bg,
            cx,
        ))
}

fn slack_composer_close_icon_layers(
    hover_group: &str,
    normal: u32,
    hover: u32,
    cross: u32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .relative()
        .size(px(20.0))
        .child(
            div()
                .absolute()
                .size_full()
                .rounded_full()
                .bg(rgb(normal))
                .flex()
                .items_center()
                .justify_center()
                .opacity(1.0)
                .group_hover(hover_group.to_string(), |style| style.opacity(0.0))
                .child(slack_icon(SlackShellIcon::Close, cross, 11.0, cx)),
        )
        .child(
            div()
                .absolute()
                .size_full()
                .rounded_full()
                .bg(rgb(hover))
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.0)
                .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
                .child(slack_icon(SlackShellIcon::Close, cross, 11.0, cx)),
        )
}

fn slack_composer_attachment_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
