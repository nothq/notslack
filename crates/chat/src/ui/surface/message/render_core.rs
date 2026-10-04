use super::hover_actions::SlackMessageHoverActionsContext;
use super::rows::SlackMessageSelectionContext;
use super::{slack_message_render_context_id, SLACK_MESSAGE_EDGE_PADDING};
use crate::ui::surface::{
    alpha, div, px, AnyElement, Context, Div, Entity, FluentBuilder, InteractiveElement,
    IntoElement, ParentElement, SlackMessageEditView, SlackMessageRenderContext, SlackMessageRow,
    StatefulInteractiveElement, Styled, SurfaceState, SLACK_MESSAGE_GAP,
};
use gpui::Role;

struct SlackMessageRowBodyContext {
    render_context: SlackMessageRenderContext,
    shows_identity: bool,
    pins_context: bool,
    hover_group: String,
    message_edit: Option<Entity<SlackMessageEditView>>,
    actions: SlackMessageHoverActionsContext,
    selection: Option<SlackMessageSelectionContext>,
}

/// A message's place in a selectable text document.
pub(crate) struct SlackMessageDocumentPosition {
    pub(crate) document_id: gpui::SharedString,
    pub(crate) row_index: usize,
    pub(crate) message_index: usize,
}

impl SlackMessageDocumentPosition {
    /// The position of the only message in a document row.
    pub(crate) fn row(document_id: gpui::SharedString, row_index: usize) -> Self {
        Self {
            document_id,
            row_index,
            message_index: 0,
        }
    }
}

struct SlackMessageRowShellContext {
    render_context: SlackMessageRenderContext,
    pins_context: bool,
    navigation_highlight: bool,
}

impl SurfaceState {
    pub(crate) fn render_slack_message(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_message_with_selection(row, render_context, None, cx)
    }

    pub(crate) fn render_slack_message_in_document(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        position: SlackMessageDocumentPosition,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackMessageDocumentPosition {
            document_id,
            row_index,
            message_index,
        } = position;
        self.render_slack_message_with_selection(
            row,
            render_context,
            Some(SlackMessageSelectionContext::new(
                document_id,
                row_index,
                message_index,
                row.id.clone(),
            )),
            cx,
        )
    }

    fn render_slack_message_with_selection(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        selection: Option<SlackMessageSelectionContext>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shows_identity = super::rows::slack_message_shows_identity(row);
        let pins_context = render_context == SlackMessageRenderContext::Pins;
        let hover_group = format!(
            "slack-message-hover-{}-{}",
            slack_message_render_context_id(render_context),
            row.id
        );
        let navigation_highlight =
            self.slack_message_is_navigation_highlight(&row.id, render_context);
        let message_edit = self.slack_message_edit_for_row(row, render_context, cx);
        let actions = SlackMessageHoverActionsContext::for_message(self, row, render_context);
        let message_row = self.render_slack_message_row_body(
            row,
            SlackMessageRowBodyContext {
                render_context,
                shows_identity,
                pins_context,
                hover_group,
                message_edit,
                actions,
                selection,
            },
            cx,
        );
        self.render_slack_message_row_shell(
            row,
            SlackMessageRowShellContext {
                render_context,
                pins_context,
                navigation_highlight,
            },
            message_row,
            cx,
        )
        .into_any_element()
    }

    fn slack_message_edit_for_row(
        &self,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
        cx: &Context<Self>,
    ) -> Option<Entity<SlackMessageEditView>> {
        (render_context == SlackMessageRenderContext::Conversation)
            .then_some(self.slack_message_edit.as_ref())
            .flatten()
            .filter(|editor| editor.read(cx).target().message_timestamp.as_str() == row.id)
            .cloned()
    }

    fn render_slack_message_row_body(
        &self,
        row: &SlackMessageRow,
        context: SlackMessageRowBodyContext,
        cx: &mut Context<Self>,
    ) -> Div {
        let editing = context.message_edit.is_some();
        div()
            .w_full()
            .min_w(px(0.0))
            .relative()
            .group(context.hover_group.clone())
            .px(px(20.0))
            .flex()
            .items_start()
            .gap(px(8.0))
            .when(editing, |this| this.h(px(96.0)).bg(alpha(0xf2c744, 0.20)))
            .when(!editing, |this| {
                this.hover(|style| style.bg(alpha(0x000000, 0.08)))
            })
            .when(row.compact, |this| {
                this.child(div().w(px(36.0)).flex_none())
            })
            .when(context.shows_identity, |this| {
                this.child(self.render_slack_message_avatar_column(row, context.pins_context, cx))
            })
            .when_some(context.message_edit, |this, editor| {
                this.child(slack_message_editor_container(editor))
            })
            .when(!editing, |this| {
                this.child(self.render_slack_message_content(
                    row,
                    context.render_context,
                    context.selection.as_ref(),
                    cx,
                ))
            })
            .when(!editing && context.actions.any_supported(), |this| {
                this.child(self.render_slack_message_hover_actions(
                    row,
                    context.hover_group,
                    context.actions,
                    cx,
                ))
            })
    }

    fn render_slack_message_avatar_column(
        &self,
        row: &SlackMessageRow,
        pins_context: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w(px(36.0))
            .flex_none()
            .pt(px(if pins_context { 0.0 } else { SLACK_MESSAGE_GAP }))
            .child(self.render_slack_message_avatar(row, cx))
    }

    fn render_slack_message_row_shell(
        &self,
        row: &SlackMessageRow,
        context: SlackMessageRowShellContext,
        message_row: Div,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(format!(
                "slack-message-row-{}-{}",
                slack_message_render_context_id(context.render_context),
                row.id
            ))
            .w_full()
            .min_w(px(0.0))
            .when(
                context.render_context == SlackMessageRenderContext::Conversation,
                |this| this.role(Role::ListItem),
            )
            .pb(px(if context.pins_context {
                0.0
            } else {
                SLACK_MESSAGE_EDGE_PADDING
            }))
            .when(context.navigation_highlight, |this| {
                this.rounded(px(4.0)).bg(alpha(0xf2c744, 0.18))
            })
            .when_some(row.divider.as_ref(), |this, divider| {
                this.child(self.render_slack_message_date_divider(divider, cx))
            })
            .when(
                row.unread_boundary_before
                    && context.render_context == SlackMessageRenderContext::Conversation,
                |this| this.child(self.render_slack_message_unread_divider()),
            )
            .child(message_row)
    }
}

fn slack_message_editor_container(editor: Entity<SlackMessageEditView>) -> Div {
    div()
        .mt(px(8.0))
        .h(px(80.0))
        .min_w(px(0.0))
        .flex_grow(1.0)
        .child(editor)
}
