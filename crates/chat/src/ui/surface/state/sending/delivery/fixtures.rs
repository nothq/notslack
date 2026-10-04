use crate::ui::surface::SurfaceState;
use crate::ui::SlackAttachment;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn slack_clip_attachment_template(&self) -> SlackAttachment {
        SlackAttachment {
            title: "Screen Recording 2026-04-04 at 3.14.15 PM.mov".to_string(),
            source: Default::default(),
            media: None,
            mimetype: "video/mp4".to_string(),
            duration_millis: None,
            description: "Recorded clip from the Slack composer.".to_string(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size: None,
            original_width: None,
            original_height: None,
            source_team_id: None,
        }
    }

    pub(in crate::ui::surface::state) fn slack_audio_attachment_template(&self) -> SlackAttachment {
        SlackAttachment {
            title: "Voice note 2026-04-04 at 3.16 PM.m4a".to_string(),
            source: Default::default(),
            media: None,
            mimetype: "audio/m4a".to_string(),
            duration_millis: None,
            description: "Recorded voice clip from the Slack composer.".to_string(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            preview_layout_size: None,
            original_width: None,
            original_height: None,
            source_team_id: None,
        }
    }
}
