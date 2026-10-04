use super::{
    draft_count_expression, local_config_credentials,
    parse::{slack_token_from_post_data, team_domain_from_api_url},
};

#[test]
fn parses_urlencoded_slack_client_token() {
    let token = slack_token_from_post_data("type=channels&token=xoxc-test&count=20")
        .expect("token should parse");
    assert_eq!(token, "xoxc-test");
}

#[test]
fn parses_multipart_slack_client_token() {
    let body = concat!(
        "------boundary\r\n",
        "Content-Disposition: form-data; name=\"token\"\r\n\r\n",
        "xoxc-test-token\r\n",
        "------boundary--\r\n"
    );
    let token = slack_token_from_post_data(body).expect("token should parse");
    assert_eq!(token, "xoxc-test-token");
}

#[test]
fn extracts_workspace_domain_from_sidebar_url() {
    let domain =
        team_domain_from_api_url("https://acme.slack.com/api/users.channelSections.list")
            .expect("domain should parse");
    assert_eq!(domain, "acme");
}

#[test]
fn extracts_local_config_web_session_for_requested_team() {
    let credentials =
        local_config_credentials(r#"{"teams":{"T123":{"domain":"acme","token":"xoxc-test"}}}"#)
            .expect("local config should parse")
            .remove(0);

    assert_eq!(credentials.team_id, "T123");
    assert_eq!(credentials.team_domain, "acme");
    assert_eq!(credentials.xoxc_token, "xoxc-test");
}

#[test]
fn extracts_local_config_user_id_for_sidebar_state() {
    let credentials = local_config_credentials(
        r#"{"teams":{"T123":{"domain":"acme","token":"xoxc-test","user_id":"U123"}}}"#,
    )
    .expect("local config should parse")
    .remove(0);

    assert_eq!(credentials.user_id.as_deref(), Some("U123"));
}

#[test]
fn local_config_extracts_all_teams() {
    let credentials = local_config_credentials(
        r#"{"teams":{"T999":{"domain":"other","token":"xoxc-test"},"T123":{"domain":"acme","token":"xoxc-test-2"}}}"#,
    )
    .expect("local config should parse");

    assert_eq!(credentials.len(), 2);
    assert!(credentials.iter().any(|session| session.team_id == "T123"));
    assert!(credentials.iter().any(|session| session.team_id == "T999"));
}

#[test]
fn draft_count_expression_targets_team_and_user_store() {
    let expression = draft_count_expression("T123", Some("U123"));

    assert!(expression.contains("persist-v1::${teamId}::"));
    assert!(expression.contains("${prefix}${userId}${suffix}"));
}
