use super::{Context, SurfaceRoot};

impl SurfaceRoot {
    pub fn activate_slack_attachment_media<AppState: 'static>(
        &mut self,
        attachment_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<String, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.activate_slack_attachment_media_by_id(attachment_id, cx)
        })
    }

    pub fn play_slack_media<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.play_slack_media(cx))
    }

    pub fn pause_slack_media<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.pause_slack_media(cx))
    }

    pub fn seek_slack_media<AppState: 'static>(
        &mut self,
        position_millis: u64,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.seek_slack_media(position_millis, cx)
        })
    }

    pub fn set_slack_media_muted<AppState: 'static>(
        &mut self,
        muted: bool,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.set_slack_media_muted(muted, cx))
    }

    pub fn stop_slack_media<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.stop_slack_media(cx))
    }

    pub fn insert_slack_composer_snippet<AppState: 'static>(
        &mut self,
        snippet: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.insert_slack_composer_snippet(snippet, cx);
        });
    }

    pub fn navigate_slack_history_back<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.navigate_slack_history_back(cx));
    }

    pub fn navigate_slack_history_forward<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.navigate_slack_history_forward(cx);
        });
    }

    pub fn can_navigate_slack_back<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.can_navigate_slack_back())
    }

    pub fn can_navigate_slack_forward<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| surface.can_navigate_slack_forward())
    }

    #[cfg(test)]
    pub fn toggle_slack_media_playback<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.toggle_slack_media_playback(attachment_title, cx);
        });
    }

    #[cfg(test)]
    pub fn toggle_slack_media_muted<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.toggle_slack_media_muted(attachment_title, cx);
        });
    }

    #[cfg(test)]
    pub fn toggle_slack_media_captions<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.toggle_slack_media_captions(attachment_title, cx);
        });
    }

    #[cfg(test)]
    pub fn cycle_slack_media_speed<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.cycle_slack_media_speed(attachment_title, cx);
        });
    }

    #[cfg(test)]
    pub fn toggle_slack_attachment_transcript<AppState: 'static>(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.toggle_slack_attachment_transcript(attachment_title, cx);
        });
    }

    pub fn toggle_slack_reaction<AppState: 'static>(
        &mut self,
        message_id: &str,
        reaction_name: &str,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.toggle_slack_reaction(message_id, reaction_name, cx);
        });
    }
}
