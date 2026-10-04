use super::{Context, Date, SurfaceState};
use crate::ui::{px, Pixels, SlackAttachment};

/// A `SurfaceState` handler bound to a control.
pub(crate) type SlackSurfaceAction = fn(&mut SurfaceState, &mut Context<SurfaceState>);

/// A labeled button that runs a `SurfaceState` action.
#[derive(Clone, Copy)]
pub(crate) struct SlackSurfaceActionButton {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) action: SlackSurfaceAction,
}

pub(crate) fn slack_base_icon_radius(size: f32) -> Pixels {
    px((size * 2.0 / 9.0).clamp(4.0, 12.0))
}

pub(crate) fn slack_attachment_date_label(attachment: &SlackAttachment) -> Option<String> {
    let start = attachment.title.find("20")?;
    let end = start.checked_add(10)?;
    let value = attachment.title.get(start..end)?;
    let date = Date::parse(
        value,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()?;
    Some(slack_date_label(date))
}

pub(crate) fn slack_date_label(date: Date) -> String {
    format!(
        "{:?}, {:?} {}{}",
        date.weekday(),
        date.month(),
        date.day(),
        ordinal_suffix(date.day())
    )
}

fn ordinal_suffix(day: u8) -> &'static str {
    match day % 100 {
        11..=13 => "th",
        _ => match day % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
}

#[cfg(test)]
pub(crate) fn slack_recording_duration_millis(attachment: &SlackAttachment) -> u32 {
    attachment
        .duration_millis
        .expect("Slack media playback requires recording duration metadata")
        .get()
}
