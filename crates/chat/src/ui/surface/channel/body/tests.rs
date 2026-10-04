use super::{slack_sticky_date_push_offset, slack_sticky_date_row_index};
use crate::ui::surface::{build_slack_message_rows, SlackMessageRow};
use crate::ui::test_support::slack_test_workspace_with_message_count;
use gpui::{px, ListOffset};

#[test]
fn slack_sticky_date_pill_replaces_scrolled_divider() {
    let rows = dated_rows();

    assert!(rows[0].divider.is_some());
    assert!(rows[0].date_label.is_some());
    assert!(rows[1].divider.is_none());
    assert!(rows[1].date_label.is_some());
    assert!(rows[2].divider.is_some());

    assert_eq!(sticky_date_row(&rows, 0, 0.0), None);
    assert_eq!(sticky_date_row(&rows, 0, 27.0), None);
    assert_eq!(sticky_date_row(&rows, 0, 28.0), Some(0));
    assert_eq!(sticky_date_row(&rows, 0, 29.0), Some(0));
    assert_eq!(
        sticky_date_row(&rows, 1, 0.0),
        Some(1),
        "a row without an in-flow divider supplies its date immediately",
    );
}

#[test]
fn slack_sticky_date_pill_ignores_sentinel_and_new_messages() {
    let rows = dated_rows();

    assert_eq!(
        slack_sticky_date_row_index(
            &rows,
            ListOffset {
                item_ix: rows.len(),
                offset_in_item: px(0.0),
            },
            0,
        ),
        None,
        "the GPUI end sentinel is not a visible row",
    );
    assert_eq!(
        slack_sticky_date_row_index(
            &rows,
            ListOffset {
                item_ix: 1,
                offset_in_item: px(0.0),
            },
            1,
        ),
        None,
        "the new-messages pill replaces the sticky date pill",
    );
}

#[test]
fn slack_sticky_date_pill_pushes_before_next_divider() {
    let overlay_top = px(100.0);
    assert_eq!(
        slack_sticky_date_push_offset(px(160.0), overlay_top),
        px(0.0),
    );
    assert_eq!(
        slack_sticky_date_push_offset(px(133.0), overlay_top),
        px(0.0),
    );
    assert_eq!(
        slack_sticky_date_push_offset(px(128.0), overlay_top),
        px(-5.0),
    );
    assert_eq!(
        slack_sticky_date_push_offset(px(110.0), overlay_top),
        px(-23.0),
    );
    assert_eq!(
        slack_sticky_date_push_offset(px(100.0), overlay_top),
        px(-33.0),
    );
    assert_eq!(
        slack_sticky_date_push_offset(px(90.0), overlay_top),
        px(-33.0),
    );
}

fn dated_rows() -> std::sync::Arc<[SlackMessageRow]> {
    let mut workspace = slack_test_workspace_with_message_count("C_AICRAZE", "design", false, 3);
    workspace.self_timezone_id = Some("UTC".to_string());
    workspace.messages[0].id = "1700000000.000000".to_string();
    workspace.messages[1].id = "1700000060.000000".to_string();
    workspace.messages[2].id = "1700086400.000000".to_string();
    build_slack_message_rows(Some(&workspace))
}

fn sticky_date_row(rows: &[SlackMessageRow], item_ix: usize, offset: f32) -> Option<usize> {
    slack_sticky_date_row_index(
        rows,
        ListOffset {
            item_ix,
            offset_in_item: px(offset),
        },
        0,
    )
}
