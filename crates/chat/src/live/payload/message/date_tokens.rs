//! Slack's date tokens, shown the way Slack shows them: a rich text `date`
//! element, or `<!date^1392734382^{date_pretty} at {time}|fallback>` in
//! mrkdwn, becomes text such as "Today at 6:39 AM" in the viewer's timezone.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{Map, Value};

/// Rewrites every date token in a raw Slack message in place.
pub(super) fn format_slack_date_tokens(message: &mut Value, timezone: Tz) {
    format_slack_date_tokens_at(message, timezone, Utc::now());
}

fn format_slack_date_tokens_at(value: &mut Value, timezone: Tz, now: DateTime<Utc>) {
    match value {
        Value::String(text) if text.contains("<!date^") => {
            *text = format_mrkdwn_date_tokens(text, timezone, now);
        }
        Value::Array(items) => {
            for item in items {
                format_slack_date_tokens_at(item, timezone, now);
            }
        }
        Value::Object(object) => {
            if let Some(text) = rich_text_date_element(object, timezone, now) {
                *value = text;
                return;
            }
            for item in object.values_mut() {
                format_slack_date_tokens_at(item, timezone, now);
            }
        }
        _ => {}
    }
}

/// A rich text `date` element as the text or link Slack draws for it.
fn rich_text_date_element(
    object: &Map<String, Value>,
    timezone: Tz,
    now: DateTime<Utc>,
) -> Option<Value> {
    if object.get("type").and_then(Value::as_str) != Some("date") {
        return None;
    }
    let timestamp = object.get("timestamp").and_then(|timestamp| {
        timestamp
            .as_i64()
            .or_else(|| timestamp.as_str()?.parse().ok())
    })?;
    let format = object
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("{date_short_pretty} at {time}");
    let text = format_slack_date(timestamp, format, timezone, now).or_else(|| {
        object
            .get("fallback")
            .and_then(Value::as_str)
            .map(str::to_string)
    })?;
    let mut element = Map::new();
    match object.get("url").and_then(Value::as_str) {
        Some(url) => {
            element.insert("type".into(), "link".into());
            element.insert("url".into(), url.into());
        }
        None => {
            element.insert("type".into(), "text".into());
        }
    }
    element.insert("text".into(), text.into());
    if let Some(style) = object.get("style") {
        element.insert("style".into(), style.clone());
    }
    Some(Value::Object(element))
}

/// Replaces each `<!date^…>` in mrkdwn with its formatted date, keeping the
/// optional link as a mrkdwn link.
fn format_mrkdwn_date_tokens(text: &str, timezone: Tz, now: DateTime<Utc>) -> String {
    let mut formatted = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!date^") {
        formatted.push_str(&rest[..start]);
        let Some(length) = rest[start..].find('>') else {
            break;
        };
        let token = &rest[start + "<!date^".len()..start + length];
        rest = &rest[start + length + 1..];
        let (spec, fallback) = token.split_once('|').unwrap_or((token, ""));
        let mut parts = spec.splitn(3, '^');
        let timestamp = parts.next().and_then(|seconds| seconds.parse::<i64>().ok());
        let format = parts.next().unwrap_or("");
        let link = parts.next().filter(|link| !link.is_empty());
        let date = timestamp
            .and_then(|timestamp| format_slack_date(timestamp, format, timezone, now))
            .unwrap_or_else(|| fallback.to_string());
        match link {
            Some(link) => formatted.push_str(&format!("<{link}|{date}>")),
            None => formatted.push_str(&date),
        }
    }
    formatted.push_str(rest);
    formatted
}

/// Fills Slack's `{date_…}` and `{time…}` placeholders; other text stays.
fn format_slack_date(
    timestamp: i64,
    format: &str,
    timezone: Tz,
    now: DateTime<Utc>,
) -> Option<String> {
    let date = timezone.timestamp_opt(timestamp, 0).single()?;
    let today = now.with_timezone(&timezone).date_naive();
    let mut formatted = String::with_capacity(format.len() + 16);
    let mut rest = format;
    while let Some(start) = rest.find('{') {
        formatted.push_str(&rest[..start]);
        let Some(length) = rest[start..].find('}') else {
            formatted.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let placeholder = &rest[start + 1..start + length];
        rest = &rest[start + length + 1..];
        match slack_date_placeholder(placeholder, &date, today, now) {
            Some(value) => formatted.push_str(&value),
            None => {
                formatted.push('{');
                formatted.push_str(placeholder);
                formatted.push('}');
            }
        }
    }
    formatted.push_str(rest);
    Some(formatted)
}

fn slack_date_placeholder(
    placeholder: &str,
    date: &DateTime<Tz>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Option<String> {
    let day = date.date_naive();
    let relative = || match (day - today).num_days() {
        0 => Some("Today".to_string()),
        -1 => Some("Yesterday".to_string()),
        1 => Some("Tomorrow".to_string()),
        _ => None,
    };
    let year = |text: String, separator: &str| {
        if day.year() == today.year() {
            text
        } else {
            format!("{text}{separator}{}", day.year())
        }
    };
    let ordinal = format!("{}{}", day.day(), ordinal_suffix(day.day()));
    let month_day = format!("{} {ordinal}", date.format("%B"));
    let long = format!("{}, {month_day}", date.format("%A"));
    let short = format!("{} {}", date.format("%b"), day.day());
    Some(match placeholder {
        "date_num" => date.format("%Y-%m-%d").to_string(),
        "date" => year(month_day, ", "),
        "date_short" => year(short, ", "),
        "date_long" => year(long, ", "),
        "date_pretty" => relative().unwrap_or_else(|| year(month_day, ", ")),
        "date_short_pretty" => relative().unwrap_or_else(|| year(short, ", ")),
        "date_long_pretty" | "day_divider_pretty" => relative().unwrap_or_else(|| year(long, ", ")),
        "time" => date.format("%-I:%M %p").to_string(),
        "time_secs" => date.format("%-I:%M:%S %p").to_string(),
        "ago" => slack_ago(date.with_timezone(&Utc), now),
        _ => return None,
    })
}

fn ordinal_suffix(day: u32) -> &'static str {
    match (day % 10, day % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    }
}

fn slack_ago(date: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - date).num_seconds();
    let (amount, unit) = match seconds.abs() {
        0..=59 => return "just now".to_string(),
        60..=3_599 => (seconds.abs() / 60, "minute"),
        3_600..=86_399 => (seconds.abs() / 3_600, "hour"),
        86_400..=2_591_999 => (seconds.abs() / 86_400, "day"),
        2_592_000..=31_535_999 => (seconds.abs() / 2_592_000, "month"),
        _ => (seconds.abs() / 31_536_000, "year"),
    };
    let plural = if amount == 1 { "" } else { "s" };
    if seconds >= 0 {
        format!("{amount} {unit}{plural} ago")
    } else {
        format!("in {amount} {unit}{plural}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::{format_mrkdwn_date_tokens, format_slack_date_tokens_at};

    #[test]
    fn mrkdwn_date_tokens_read_like_slack() {
        let timezone = chrono_tz::Asia::Manila;
        // Tuesday, November 17th 2026, 7:45 AM in Manila.
        let now = Utc.with_ymd_and_hms(2026, 11, 16, 23, 45, 0).unwrap();
        // 10:30 PM-11:00 PM on Friday, November 13th in Manila.
        let start = Utc
            .with_ymd_and_hms(2026, 11, 13, 14, 30, 0)
            .unwrap()
            .timestamp();
        let end = start + 30 * 60;
        let text = format!(
            "<!date^{start}^{{time}}|{start}>-<!date^{end}^{{time}}|{end}> <https://x.test|FE Daily Meeting>"
        );
        assert_eq!(
            format_mrkdwn_date_tokens(&text, timezone, now),
            "10:30 PM-11:00 PM <https://x.test|FE Daily Meeting>"
        );
        assert_eq!(
            format_mrkdwn_date_tokens(
                &format!("<!date^{start}^{{date_long_pretty}}^https://x.test|2026-11-13> Home"),
                timezone,
                now
            ),
            "<https://x.test|Friday, November 13th> Home"
        );
        let today = Utc
            .with_ymd_and_hms(2026, 11, 17, 1, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            format_mrkdwn_date_tokens(
                &format!("<!date^{today}^{{date_long_pretty}}|x>"),
                timezone,
                now
            ),
            "Today"
        );
        assert_eq!(
            format_mrkdwn_date_tokens("<!date^nope^{time}|later>", timezone, now),
            "later"
        );
    }

    #[test]
    fn rich_text_date_elements_become_text() {
        let timezone = chrono_tz::Asia::Manila;
        let now = Utc.with_ymd_and_hms(2026, 11, 16, 23, 45, 0).unwrap();
        let start = Utc
            .with_ymd_and_hms(2026, 11, 13, 14, 30, 0)
            .unwrap()
            .timestamp();
        let mut message = json!({"blocks": [{"type": "rich_text", "elements": [{
            "type": "rich_text_section",
            "elements": [
                {"type": "text", "text": "Today-"},
                {"type": "date", "timestamp": start, "format": "{date_long_pretty}", "fallback": "x"}
            ]
        }]}]});
        format_slack_date_tokens_at(&mut message, timezone, now);
        assert_eq!(
            message["blocks"][0]["elements"][0]["elements"][1],
            json!({"type": "text", "text": "Friday, November 13th"})
        );
    }
}
