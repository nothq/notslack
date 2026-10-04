use crate::ui::surface::message::rows::*;
use crate::ui::surface::{SlackAttachmentRenderKind, SlackMessageBody, SlackMessageRow};
use crate::ui::SlackAttachment;

#[gpui::test]
fn slack_reaction_display_resolves_standard_shortcodes() {
    assert_eq!(slack_reaction_display("white_check_mark"), "✅");
    assert_eq!(slack_reaction_display("thumbsup::skin-tone-3"), "👍🏼");
}

#[gpui::test]
fn slack_reaction_display_falls_back_for_unknown_shortcodes() {
    assert_eq!(slack_reaction_display("party-parrot"), ":party-parrot:");
}

#[gpui::test]
fn slack_message_chunks_group_simple_histories() {
    let rows = (0..12)
        .map(|index| SlackMessageRow {
            id: format!("M{index}"),
            author: "notslack".to_string(),
            timestamp: "10:00 AM".to_string(),
            compact: false,
            action_target: None,
            delivery: None,
            saved_state: None,
            user_id: None,
            avatar_text: "O".to_string(),
            avatar_fill: 0,
            avatar_image_url: None,
            body: SlackMessageBody {
                text: format!("Message {index}").into(),
                ..Default::default()
            },
            edit_round_trip_supported: true,
            table_rows: Vec::new(),
            edited_label: None,
            local_date: None,
            divider: None,
            unread_boundary_before: false,
            date_label: None,
            attachments: Vec::new(),
            attachment_layout: Default::default(),
            reaction_state: Default::default(),
            reactions: Vec::new(),
            reply_count: None,
            latest_reply_timestamp: None,
            reply_summary: None,
            reply_participant_user_ids: Default::default(),
            reply_participants: Default::default(),
            latest_reply_author: None,
            latest_reply_user_id: None,
            latest_reply_avatar_text: None,
            latest_reply_avatar_fill: 0,
            latest_reply_avatar_image_url: None,
            replies: Vec::new(),
        })
        .collect::<Vec<_>>();

    let chunks = build_slack_message_chunks(&rows);

    assert!(chunks.len() < rows.len());
    assert_eq!(
        chunks.first().map(|chunk| chunk.row_range.clone()),
        Some(0..10)
    );
}

#[gpui::test]
fn slack_attachment_row_drops_embedded_preview_payload_after_caching_key() {
    let row = slack_attachment_row(
        &SlackAttachment {
            title: "Screenshot".to_string(),
            source: Default::default(),
            mimetype: "image/png".to_string(),
            description: String::new(),
            link_url: "https://example.com/screenshot.png".to_string(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: Some("ZmFrZQ==".to_string()),
            preview_image_mimetype: Some("image/png".to_string()),
            ..SlackAttachment::default()
        },
        chrono_tz::UTC,
    );

    assert!(row.preview_cache_key.is_some());
    assert!(row.attachment.preview_image_base64.is_none());
    assert!(row.attachment.preview_image_mimetype.is_none());
}

#[gpui::test]
fn slack_recording_row_prepares_real_duration_and_preview_geometry() {
    let row = slack_attachment_row(
        &SlackAttachment {
            title: "Screen Recording 2026-07-24 at 9.40.27 PM.mov".to_string(),
            source: Default::default(),
            mimetype: "video/quicktime".to_string(),
            duration_millis: std::num::NonZeroU32::new(18_000),
            preview_image_url: Some(
                "https://files.slack.com/files-tmb/T_TEST-F_TEST/video.png".to_string(),
            ),
            preview_layout_size: crate::ui::SlackAttachmentPreviewSize::new(171, 354),
            ..SlackAttachment::default()
        },
        chrono_tz::UTC,
    );

    assert_eq!(row.kind, SlackAttachmentRenderKind::Recording);
    assert_eq!(row.recording_duration_label.as_deref(), Some("0:18"));
    let preview_size = row
        .preview_size
        .expect("recording preview geometry should be prepared");
    assert_eq!(preview_size.width(), 171);
    assert_eq!(preview_size.height(), 354);
}
