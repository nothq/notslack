use std::fs;

use crate::model::SlackMessage;

use super::SlackWorkspaceCacheStore;

#[test]
fn confirmed_sends_survive_an_encrypted_cache_restart() {
    let cache_root = tempfile::tempdir().expect("cache tempdir should be available");
    let key = [0x5a; 32];
    let message = message("1700000000.000001", "restart secret");
    let store = SlackWorkspaceCacheStore::for_test(cache_root.path(), "T_TEST", key);

    store.persist_confirmed_sends("C_TEST", std::slice::from_ref(&message));

    let encrypted = fs::read(store.paths.confirmed_sends_path("C_TEST"))
        .expect("confirmed-send cache should exist");
    assert!(!encrypted
        .windows(message.body.len())
        .any(|window| window == message.body.as_bytes()));

    let restarted = SlackWorkspaceCacheStore::for_test(cache_root.path(), "T_TEST", key);
    assert_eq!(
        restarted
            .load_confirmed_sends("C_TEST")
            .expect("confirmed sends should decrypt after restart"),
        [message]
    );
}

fn message(id: &str, body: &str) -> SlackMessage {
    SlackMessage {
        id: id.to_string(),
        client_message_id: None,
        author: "You".to_string(),
        timestamp: "Now".to_string(),
        user_id: Some("U_SELF".to_string()),
        avatar_label: None,
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: body.to_string(),
        rich_body: None,
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: None,
        attachments: Vec::new(),
        reactions: Vec::new(),
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    }
}
