use gpui::{
    div, px, Context, Div, InteractiveElement, Stateful, StatefulInteractiveElement, Styled, Window,
};

use super::super::{SlackVideoClipModal, SlackVideoClipModalPhase};

impl SlackVideoClipModal {
    pub(super) fn initialize_focus_containment(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.focus_subscriptions.is_empty() {
            let start = self.focus_guard_start.clone();
            self.focus_subscriptions
                .push(cx.on_focus(&start, window, |this, window, cx| {
                    let focus = this.last_control_focus_handle();
                    window.focus(&focus, cx);
                }));
            let end = self.focus_guard_end.clone();
            self.focus_subscriptions
                .push(cx.on_focus(&end, window, |this, window, cx| {
                    let focus = this.first_control_focus_handle();
                    window.focus(&focus, cx);
                }));
        }
        if std::mem::take(&mut self.focus_pending) {
            let focus = self.first_control_focus_handle();
            window.focus(&focus, cx);
        }
    }

    pub(super) fn render_start_focus_guard(&self) -> Stateful<Div> {
        self.render_focus_guard("slack-video-clip-focus-start", &self.focus_guard_start, 0)
    }

    pub(super) fn render_end_focus_guard(&self) -> Stateful<Div> {
        self.render_focus_guard("slack-video-clip-focus-end", &self.focus_guard_end, 100)
    }

    pub(super) fn close_available(&self) -> bool {
        !matches!(
            self.phase,
            SlackVideoClipModalPhase::Finalizing
                | SlackVideoClipModalPhase::Attaching
                | SlackVideoClipModalPhase::Cancelling
        )
    }

    fn render_focus_guard(
        &self,
        id: &'static str,
        focus_handle: &gpui::FocusHandle,
        tab_index: isize,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .absolute()
            .size(px(0.0))
            .overflow_hidden()
            .focusable()
            .track_focus(focus_handle)
            .tab_index(tab_index)
            .tab_stop(true)
    }

    fn first_control_focus_handle(&self) -> gpui::FocusHandle {
        if self.discard_confirmation {
            return self.keep_focus_handle.clone();
        }
        if self.close_available() {
            return self.close_focus_handle.clone();
        }
        self.status_focus_handle.clone()
    }

    fn last_control_focus_handle(&self) -> gpui::FocusHandle {
        if self.discard_confirmation {
            return self.discard_focus_handle.clone();
        }
        match self.phase {
            SlackVideoClipModalPhase::Preparing => self.upload_focus_handle.clone(),
            SlackVideoClipModalPhase::Previewing if self.countdown.is_some() => {
                self.record_action_focus_handle.clone()
            }
            SlackVideoClipModalPhase::Previewing => self.record_action_focus_handle.clone(),
            SlackVideoClipModalPhase::Starting => self.record_action_focus_handle.clone(),
            SlackVideoClipModalPhase::Recording => self.record_action_focus_handle.clone(),
            SlackVideoClipModalPhase::Reviewing => self.done_focus_handle.clone(),
            SlackVideoClipModalPhase::Failed => self.close_focus_handle.clone(),
            SlackVideoClipModalPhase::Finalizing
            | SlackVideoClipModalPhase::Attaching
            | SlackVideoClipModalPhase::Cancelling => self.status_focus_handle.clone(),
        }
    }
}
