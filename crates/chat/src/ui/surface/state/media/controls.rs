#[cfg(test)]
use std::time::Duration;

#[cfg(test)]
use crate::ui::surface::slack_recording_duration_millis;
use crate::ui::surface::SurfaceState;
use crate::ui::Context;

impl SurfaceState {
    #[cfg(test)]
    pub(crate) fn toggle_slack_media_playback(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<Self>,
    ) {
        let duration_millis = self.slack_attachment_duration_millis(attachment_title);
        let state = self
            .slack_media_states
            .entry(attachment_title.to_string())
            .or_default();
        state.playing = !state.playing;
        if state.playing && state.progress_millis >= duration_millis {
            state.progress_millis = 0;
        }
        let should_tick = state.playing;
        cx.notify();
        if should_tick {
            self.spawn_slack_media_playback_tick(attachment_title.to_string(), cx);
        }
    }

    #[cfg(test)]
    pub(crate) fn toggle_slack_media_muted(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<Self>,
    ) {
        let state = self
            .slack_media_states
            .entry(attachment_title.to_string())
            .or_default();
        state.muted = !state.muted;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn toggle_slack_media_captions(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<Self>,
    ) {
        let state = self
            .slack_media_states
            .entry(attachment_title.to_string())
            .or_default();
        state.captions_enabled = !state.captions_enabled;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn toggle_slack_attachment_transcript(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<Self>,
    ) {
        let state = self
            .slack_media_states
            .entry(attachment_title.to_string())
            .or_default();
        state.transcript_generated = true;
        state.transcript_visible = !state.transcript_visible;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn cycle_slack_media_speed(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<Self>,
    ) {
        let state = self
            .slack_media_states
            .entry(attachment_title.to_string())
            .or_default();
        state.playback_speed = state.playback_speed.next();
        cx.notify();
    }

    #[cfg(test)]
    fn spawn_slack_media_playback_tick(
        &mut self,
        attachment_title: String,
        cx: &mut Context<Self>,
    ) {
        self.spawn_timer_task(
            attachment_title,
            Duration::from_millis(1_000),
            cx,
            |this, attachment_title, cx| {
                this.advance_slack_media_playback(&attachment_title, cx);
            },
        );
    }

    #[cfg(test)]
    fn advance_slack_media_playback(&mut self, attachment_title: &str, cx: &mut Context<Self>) {
        let duration_millis = self.slack_attachment_duration_millis(attachment_title);
        let Some(state) = self.slack_media_states.get_mut(attachment_title) else {
            return;
        };
        if !state.playing {
            return;
        }
        state.progress_millis =
            (state.progress_millis + state.playback_speed.step_millis()).min(duration_millis);
        if state.progress_millis >= duration_millis {
            state.playing = false;
        }
        cx.notify();
        if state.playing {
            self.spawn_slack_media_playback_tick(attachment_title.to_string(), cx);
        }
    }

    #[cfg(test)]
    fn slack_attachment_duration_millis(&self, attachment_title: &str) -> u32 {
        self.slack_workspace()
            .and_then(|workspace| {
                workspace
                    .messages
                    .iter()
                    .flat_map(|message| message.attachments.iter())
                    .find(|attachment| attachment.title == attachment_title)
            })
            .map(slack_recording_duration_millis)
            .expect("Slack media test state must reference a current workspace attachment")
    }

    pub(crate) fn toggle_slack_attachment_collapsed(
        &mut self,
        attachment_id: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_collapsed_attachment_ids.remove(attachment_id) {
            self.slack_collapsed_attachment_ids
                .insert(attachment_id.to_string());
        }
        self.slack_message_list_state.remeasure();
        if let Some(thread_panel) = self.slack_thread_panel.as_mut() {
            thread_panel.list_state.remeasure();
        }
        cx.notify();
    }
}
