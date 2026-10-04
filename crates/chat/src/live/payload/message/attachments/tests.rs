use std::collections::HashMap;

use serde_json::json;

use crate::live::payload::message::attachments::*;
use crate::live::payload::message::slack_message_from_value;

#[gpui::test]
fn slack_attachment_metadata_preserves_unfurl_fields() {
    let attachment = slack_attachment_metadata_from_message_attachment(&json!({
        "title": "Zed adds a new Slack mode",
        "title_link": "https://zed.dev/blog/slack-mode",
        "text": "Detailed write-up about the new Slack workflow in Zed.",
        "service_name": "Zed",
    }))
    .expect("expected attachment metadata");

    assert_eq!(attachment.title, "Zed adds a new Slack mode");
    assert_eq!(attachment.link_url, "https://zed.dev/blog/slack-mode");
    assert_eq!(
        attachment.description,
        "Detailed write-up about the new Slack workflow in Zed."
    );
    assert_eq!(attachment.source_label, "Zed");
    assert!(attachment.is_website_preview());
}

#[gpui::test]
fn slack_attachment_metadata_uses_link_host_when_service_name_is_missing() {
    let attachment = slack_attachment_metadata_from_message_attachment(&json!({
        "title": "Launch target registry notes",
        "original_url": "https://example.com/blog/launch-targets",
        "text": "Launch target registry notes",
    }))
    .expect("expected attachment metadata");

    assert_eq!(attachment.source_label, "example.com");
    assert_eq!(attachment.description, "");
}

#[gpui::test]
fn slack_file_attachment_preserves_recording_metadata() {
    let attachment = slack_attachment_from_file(&json!({
        "title": "Screen Recording 2026-07-24 at 9.40.27 PM.mov",
        "mimetype": "video/quicktime",
        "duration_ms": 18_000,
        "permalink": "https://acme.slack.com/files/U_TEST/F_TEST",
        "thumb_video": "https://files.slack.com/files-tmb/T_TEST-F_TEST/video.png",
        "thumb_360_w": 171,
        "thumb_360_h": 354,
        "original_w": 1_080,
        "original_h": 1_920
    }))
    .expect("expected recording attachment");

    assert_eq!(
        attachment.duration_millis.map(|duration| duration.get()),
        Some(18_000)
    );
    assert_eq!(
        attachment.preview_image_url.as_deref(),
        Some("https://files.slack.com/files-tmb/T_TEST-F_TEST/video.png")
    );
    let preview_size = attachment
        .preview_layout_size
        .expect("recording thumbnail dimensions should be preserved");
    assert_eq!(preview_size.width(), 171);
    assert_eq!(preview_size.height(), 354);
}

#[gpui::test]
fn slack_message_extracts_image_block_attachments() {
    let message = slack_message_from_value(
        json!({
            "ts": "1775476884.547799",
            "subtype": "bot_message",
            "username": "MetaBot",
            "text": "fallback text that should not be primary",
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": "Yesterday"
                    }
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": "<https://dashboards.example.com/dashboard/1420#scrollTo=3155|TOTAL VOL by geo>"
                    }
                },
                {
                    "type": "image",
                    "alt_text": "TOTAL VOL by geo",
                    "image_url": "https://files.slack.com/files-pri/TQRADTDAL-F0ARFJEHPHP/total_vol_by_geo.png",
                    "image_bytes": 186489
                }
            ]
        }),
        &HashMap::new(),
    );

    assert_eq!(message.body, "Yesterday");
    assert_eq!(message.attachments.len(), 1);
    assert_eq!(message.attachments[0].title, "TOTAL VOL by geo");
    assert_eq!(
        message.attachments[0].preview_image_url.as_deref(),
        Some("https://files.slack.com/files-pri/TQRADTDAL-F0ARFJEHPHP/total_vol_by_geo.png")
    );
    assert_eq!(
        message.attachments[0].link_url,
        "https://dashboards.example.com/dashboard/1420#scrollTo=3155"
    );
    assert_eq!(message.attachments[0].description, "(187 kB)");
}
