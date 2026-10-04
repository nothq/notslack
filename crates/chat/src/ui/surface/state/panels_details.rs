mod self_panel;

#[cfg(test)]
use super::SlackMediaState;
use super::{Context, SurfaceState};
#[cfg(test)]
use super::{SlackAuxPanelRow, SlackAuxPanelRowAction, SlackAuxPanelSection, SlackAuxPanelState};
#[cfg(test)]
use crate::ui::surface::SlackAttachmentSelection;

enum SlackExternalConnectionTarget {
    Conversation(String),
    Profile(String),
}

impl SurfaceState {
    pub(crate) fn open_slack_external_connection(
        &mut self,
        name: &str,
        _company: &str,
        cx: &mut Context<Self>,
    ) {
        match self.slack_visible_external_connection_target(name) {
            Some(SlackExternalConnectionTarget::Conversation(conversation_id)) => {
                self.select_slack_conversation(&conversation_id, cx);
            }
            Some(SlackExternalConnectionTarget::Profile(user_id)) => {
                self.open_slack_profile(&user_id, cx);
            }
            None if self.slack_workspace_api_capabilities.search_messages => {
                self.open_slack_search_results(name, cx);
            }
            None => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn open_slack_attachment_menu(
        &mut self,
        attachment_title: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.resolve_slack_attachment_selection_by_title(attachment_title)
        else {
            return;
        };
        self.open_slack_attachment_menu_for_selection(&selection, cx);
    }

    #[cfg(test)]
    fn open_slack_attachment_menu_for_selection(
        &mut self,
        selection: &SlackAttachmentSelection,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        let media_state = self.slack_media_state(selection.title.as_ref());
        self.set_slack_aux_panel(
            SlackAuxPanelState {
                title: "Attachment actions".to_string(),
                subtitle: Some(selection.title.to_string()),
                query: None,
                query_behavior: None,
                sections: vec![SlackAuxPanelSection {
                    title: Some("Playback".to_string()),
                    rows: self.slack_attachment_menu_rows(
                        selection,
                        #[cfg(test)]
                        &media_state,
                    ),
                }],
            },
            cx,
        );
    }

    pub(crate) fn open_slack_self_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(user_id) = self.slack_self_identity().1 {
            self.open_slack_profile(&user_id, cx);
        }
    }

    fn slack_visible_external_connection_target(
        &self,
        name: &str,
    ) -> Option<SlackExternalConnectionTarget> {
        self.slack_workspace().and_then(|workspace| {
            workspace
                .sections
                .iter()
                .flat_map(|section| section.items.iter())
                .find(|item| item.label.eq_ignore_ascii_case(name))
                .and_then(|item| {
                    (!item.target_id.is_empty())
                        .then(|| {
                            SlackExternalConnectionTarget::Conversation(item.target_id.clone())
                        })
                        .or_else(|| {
                            item.user_id
                                .clone()
                                .map(SlackExternalConnectionTarget::Profile)
                        })
                })
        })
    }

    #[cfg(test)]
    fn slack_attachment_menu_rows(
        &self,
        selection: &SlackAttachmentSelection,
        media_state: &SlackMediaState,
    ) -> Vec<SlackAuxPanelRow> {
        let attachment_title = selection.title.as_ref();
        vec![
            Self::slack_aux_row(
                if media_state.playing {
                    "Pause preview"
                } else {
                    "Play preview"
                },
                Some("Toggle the inline playback state for this recording.".to_string()),
                None,
                false,
                Some(SlackAuxPanelRowAction::ToggleAttachmentPlayback(
                    attachment_title.to_string(),
                )),
            ),
            Self::slack_aux_row(
                if media_state.transcript_visible {
                    "Hide transcript"
                } else if media_state.transcript_generated {
                    "Show transcript"
                } else {
                    "Generate transcript"
                },
                Some("Open the generated transcript panel for this attachment.".to_string()),
                None,
                false,
                Some(SlackAuxPanelRowAction::ToggleAttachmentTranscript(
                    attachment_title.to_string(),
                )),
            ),
            Self::slack_aux_row(
                format!("Playback speed {}", media_state.playback_speed.label()),
                Some("Cycle between 1x, 1.5x, and 2x playback.".to_string()),
                None,
                false,
                Some(SlackAuxPanelRowAction::CycleAttachmentPlaybackSpeed(
                    attachment_title.to_string(),
                )),
            ),
            Self::slack_aux_row(
                "Open preview",
                Some("Expand this attachment into the lightbox preview.".to_string()),
                None,
                false,
                Some(SlackAuxPanelRowAction::ToggleAttachmentExpanded(
                    selection.attachment_id.to_string(),
                )),
            ),
        ]
    }
}
