use super::{SlackAuxPanelRow, SlackAuxPanelRowAction, SurfaceState};

impl SurfaceState {
    pub(crate) fn slack_aux_label_row(label: impl Into<String>) -> SlackAuxPanelRow {
        SlackAuxPanelRow {
            label: label.into(),
            emoji_glyph: None,
            detail: None,
            accessory: None,
            image_url: None,
            image_base64: None,
            image_mimetype: None,
            muted: false,
            action: None,
        }
    }

    pub(crate) fn slack_aux_row(
        label: impl Into<String>,
        detail: Option<String>,
        accessory: Option<String>,
        muted: bool,
        action: Option<SlackAuxPanelRowAction>,
    ) -> SlackAuxPanelRow {
        SlackAuxPanelRow {
            label: label.into(),
            emoji_glyph: None,
            detail,
            accessory,
            image_url: None,
            image_base64: None,
            image_mimetype: None,
            muted,
            action,
        }
    }
}
