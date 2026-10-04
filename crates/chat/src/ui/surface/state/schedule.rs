mod actions;
mod calendar;
mod edit;
mod inputs;
mod menu;
mod overlay;
mod owner;
mod parse;
mod picker_actions;
mod picker_navigation;
mod recovery;
mod submit;

use calendar::{
    slack_schedule_calendar_cells, slack_schedule_date_in_month, slack_schedule_input_style,
    slack_schedule_month_start, slack_schedule_time_cursor_index, slack_schedule_time_options,
};
use menu::{slack_schedule_menu_state, stop_slack_schedule_key_event};
use parse::{
    next_weekday, parse_slack_schedule_date_input, parse_slack_schedule_time_input,
    slack_schedule_date_label, slack_schedule_month_full_label, slack_schedule_time_label,
    slack_schedule_weekday_full_label,
};
use std::{rc::Rc, sync::Arc};

use chrono::{
    Datelike, Days, LocalResult, Months, NaiveDate, NaiveTime, TimeZone, Timelike, Utc, Weekday,
};
use chrono_tz::Tz;
use gpui::{Entity, KeyDownEvent, ScrollStrategy, SharedString, UniformListScrollHandle, Window};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::super::{alpha, px, rgb};
use super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackActiveScheduledEdit, SlackComposerDestination, SlackComposerDocument, SlackComposerDraft,
    SlackComposerDraftKey, SlackComposerFileState, SlackFileStagingLocator,
    SlackMainComposerDraftHandle, SlackMainComposerDraftOwner, SlackMainRoute,
    SlackRemoteDraftFileLocator, SlackScheduleDraftOwner, SlackScheduledDraftDisposition,
    SlackScheduledEdit, SlackSendDraftSource, SlackThreadDraftHandle, SlackThreadPanelState,
};

const SLACK_SCHEDULE_MORNING_HOUR: u32 = 9;
const SLACK_SCHEDULE_EVENING_HOUR: u32 = 18;
const SLACK_SCHEDULE_CUSTOM_STEP_MINUTES: u32 = 15;
const SLACK_SCHEDULE_CALENDAR_CELL_COUNT: usize = 42;
const SLACK_SCHEDULE_MAX_DAYS: u64 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackSchedulePostAt(i64);

impl SlackSchedulePostAt {
    fn from_local(timezone: Tz, date: NaiveDate, time: NaiveTime) -> Result<Self, String> {
        match timezone.from_local_datetime(&date.and_time(time)) {
            LocalResult::Single(value) => Ok(Self(value.timestamp())),
            LocalResult::Ambiguous(earliest, _) => Ok(Self(earliest.timestamp())),
            LocalResult::None => Err(format!(
                "The selected time does not exist in the {} time zone.",
                timezone
            )),
        }
    }

    fn future_unix_seconds(self) -> Result<i64, String> {
        (self.0 > Utc::now().timestamp())
            .then_some(self.0)
            .ok_or_else(|| "Choose a future time for this scheduled message.".to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSchedulePreset {
    pub(crate) label: String,
    pub(crate) post_at: SlackSchedulePostAt,
}

#[derive(Clone, Debug)]
pub(crate) struct SlackScheduleMenuState {
    pub(crate) presets: [Option<SlackSchedulePreset>; 3],
    pub(crate) custom: SlackScheduleCustomState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackScheduleCalendarCellStatus {
    Blank,
    Unavailable,
    Available,
    Selected,
}

impl SlackScheduleCalendarCellStatus {
    pub(crate) fn is_selectable(self) -> bool {
        matches!(self, Self::Available | Self::Selected)
    }

    pub(crate) fn is_selected(self) -> bool {
        self == Self::Selected
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackScheduleCalendarCell {
    pub(crate) date: Option<NaiveDate>,
    pub(crate) day_label: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) status: SlackScheduleCalendarCellStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackScheduleDatePickerState {
    pub(crate) displayed_month: NaiveDate,
    pub(crate) month_label: SharedString,
    pub(crate) cells: Arc<[SlackScheduleCalendarCell]>,
    pub(crate) cursor_date: NaiveDate,
    pub(crate) can_show_previous_month: bool,
    pub(crate) can_show_next_month: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackScheduleTimeOption {
    pub(crate) time: NaiveTime,
    pub(crate) label: SharedString,
}

#[derive(Clone, Debug)]
pub(crate) struct SlackScheduleTimePickerState {
    pub(crate) options: Arc<[SlackScheduleTimeOption]>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) cursor_index: usize,
    pub(crate) scroll_handle: UniformListScrollHandle,
}

#[derive(Clone, Debug)]
pub(crate) enum SlackScheduleNestedPicker {
    Date(SlackScheduleDatePickerState),
    Time(SlackScheduleTimePickerState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlackScheduleMonthDirection {
    Previous,
    Next,
}

#[derive(Clone, Debug)]
pub(crate) struct SlackScheduleCustomState {
    pub(crate) timezone: Tz,
    pub(crate) timezone_label: String,
    pub(crate) opened_date: NaiveDate,
    pub(crate) maximum_date: NaiveDate,
    pub(crate) date: NaiveDate,
    pub(crate) time: NaiveTime,
    pub(crate) date_label: String,
    pub(crate) time_label: String,
    pub(crate) date_edit_text: String,
    pub(crate) time_edit_text: String,
    pub(crate) date_input_focused: bool,
    pub(crate) time_input_focused: bool,
    pub(crate) error: Option<String>,
    pub(crate) picker: Option<SlackScheduleNestedPicker>,
}

impl SlackScheduleCustomState {
    fn set_date(&mut self, date: NaiveDate) {
        self.date = date;
        self.date_label = slack_schedule_date_label(self.opened_date, date);
        self.date_edit_text.clone_from(&self.date_label);
        self.error = None;
    }

    fn set_time(&mut self, time: NaiveTime) {
        self.time = time;
        self.time_label = slack_schedule_time_label(time);
        self.time_edit_text.clone_from(&self.time_label);
        self.error = None;
    }

    fn post_at(&self) -> Result<SlackSchedulePostAt, String> {
        SlackSchedulePostAt::from_local(self.timezone, self.date, self.time)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SlackScheduleOverlay {
    Menu(SlackScheduleMenuState),
    Custom(SlackScheduleCustomState),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SlackScheduleAnchor {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SlackScheduleOverlayState {
    pub(crate) owner: SlackScheduleDraftOwner,
    pub(crate) anchor: SlackScheduleAnchor,
    pub(crate) phase: SlackScheduleOverlay,
}

impl SlackScheduleOverlayState {
    pub(crate) fn custom(&self) -> Option<&SlackScheduleCustomState> {
        match &self.phase {
            SlackScheduleOverlay::Menu(_) => None,
            SlackScheduleOverlay::Custom(custom) => Some(custom),
        }
    }

    pub(crate) fn custom_mut(&mut self) -> Option<&mut SlackScheduleCustomState> {
        match &mut self.phase {
            SlackScheduleOverlay::Menu(_) => None,
            SlackScheduleOverlay::Custom(custom) => Some(custom),
        }
    }
}

#[derive(Clone, Copy)]
struct SlackScheduleCalendarDates {
    opened: NaiveDate,
    maximum: NaiveDate,
    selected: NaiveDate,
}

impl SlackScheduleDatePickerState {
    fn new(opened_date: NaiveDate, maximum_date: NaiveDate, selected_date: NaiveDate) -> Self {
        let displayed_month = slack_schedule_month_start(selected_date);
        let mut picker = Self {
            displayed_month,
            month_label: SharedString::default(),
            cells: Arc::default(),
            cursor_date: selected_date,
            can_show_previous_month: false,
            can_show_next_month: false,
        };
        picker.rebuild(
            displayed_month,
            selected_date,
            SlackScheduleCalendarDates {
                opened: opened_date,
                maximum: maximum_date,
                selected: selected_date,
            },
        );
        picker
    }

    fn rebuild(
        &mut self,
        displayed_month: NaiveDate,
        cursor_date: NaiveDate,
        dates: SlackScheduleCalendarDates,
    ) {
        let minimum_month = slack_schedule_month_start(dates.opened);
        let maximum_month = slack_schedule_month_start(dates.maximum);
        self.displayed_month = displayed_month;
        self.month_label = format!(
            "{} {}",
            slack_schedule_month_full_label(displayed_month.month()),
            displayed_month.year()
        )
        .into();
        self.cells = slack_schedule_calendar_cells(
            displayed_month,
            dates.opened,
            dates.maximum,
            dates.selected,
        );
        self.cursor_date = cursor_date;
        self.can_show_previous_month = displayed_month > minimum_month;
        self.can_show_next_month = displayed_month < maximum_month;
    }
}

#[derive(Clone)]
pub(super) struct SlackScheduleDraftIdentity {
    pub(super) key: Option<SlackComposerDraftKey>,
    pub(super) destination: SlackComposerDestination,
    pub(super) team_id: String,
    pub(super) self_user_id: String,
    pub(super) conversation_id: String,
}

pub(super) enum SlackScheduleDraftRestore {
    Active,
    Stored,
}

pub(super) fn slack_remote_loading_file_locators(
    owner: &SlackScheduleDraftOwner,
    draft: &SlackComposerDraft,
) -> Vec<SlackRemoteDraftFileLocator> {
    assert_eq!(
        owner.draft_id(),
        draft.id,
        "Slack remote scheduled-draft files require their exact typed owner"
    );
    let file_owner = owner.file_staging_owner();
    draft
        .files
        .iter()
        .filter_map(|file| match file.state() {
            SlackComposerFileState::RemoteLoading { reference } => {
                Some(SlackRemoteDraftFileLocator {
                    owner: file_owner.clone(),
                    draft_id: draft.id,
                    file_id: file.id(),
                    reference: reference.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

fn slack_schedule_draft_is_ready(draft: &SlackComposerDraft) -> bool {
    slack_schedule_draft_has_content(draft) && draft.files.slack_file_ids_ready()
}

fn slack_schedule_draft_has_content(draft: &SlackComposerDraft) -> bool {
    draft.document.has_message_content() || !draft.files.is_empty()
}
