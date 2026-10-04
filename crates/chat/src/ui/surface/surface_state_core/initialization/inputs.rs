use gpui::{AppContext, Context, ElementId, Entity};
use gpui_components::text_input::{TextInput, TextInputProps};

use super::SurfaceState;

pub(super) struct SlackTextInput {
    pub(super) entity: Entity<TextInput>,
    pub(super) accessibility_id: ElementId,
}

pub(super) struct SlackSurfaceInputs {
    pub(super) search: SlackTextInput,
    pub(super) files_search: SlackTextInput,
    pub(super) conversation_files_search: SlackTextInput,
    pub(super) members_search: SlackTextInput,
    pub(super) directory_search: SlackTextInput,
    pub(super) home_finder: SlackTextInput,
    pub(super) dm_finder: SlackTextInput,
    pub(super) new_message_to: SlackTextInput,
    pub(super) forward_destination: SlackTextInput,
    pub(super) forward_note: SlackTextInput,
    pub(super) self_status_text: SlackTextInput,
    pub(super) self_status_emoji: SlackTextInput,
    pub(super) sidebar_section_name: SlackTextInput,
    pub(super) later_reminder: SlackTextInput,
    pub(super) reaction_picker_search: SlackTextInput,
    pub(super) composer: SlackTextInput,
    pub(super) thread_composer: SlackTextInput,
    pub(super) link_text: SlackTextInput,
    pub(super) link_url: SlackTextInput,
    pub(super) schedule_date: SlackTextInput,
    pub(super) schedule_time: SlackTextInput,
}

struct SlackSearchInputs {
    search: SlackTextInput,
    files: SlackTextInput,
    conversation_files: SlackTextInput,
    members: SlackTextInput,
}

impl SlackSearchInputs {
    fn new(cx: &mut Context<SurfaceState>) -> Self {
        Self {
            search: new_slack_text_input(
                cx,
                "slack-message-search",
                "Search Slack messages",
                false,
            ),
            files: new_slack_text_input(cx, "slack-files-search", "Search Slack files", false),
            conversation_files: new_slack_text_input(
                cx,
                "slack-conversation-files-search",
                "Search conversation files and links",
                false,
            ),
            members: new_slack_text_input(
                cx,
                "slack-members-search",
                "Find channel members",
                false,
            ),
        }
    }
}

struct SlackNavigationInputs {
    directory_search: SlackTextInput,
    home_finder: SlackTextInput,
    dm_finder: SlackTextInput,
    new_message_to: SlackTextInput,
    sidebar_section_name: SlackTextInput,
    later_reminder: SlackTextInput,
}

impl SlackNavigationInputs {
    fn new(cx: &mut Context<SurfaceState>) -> Self {
        Self {
            directory_search: new_slack_text_input(
                cx,
                "slack-directory-search",
                "Search people",
                false,
            ),
            home_finder: new_slack_text_input(
                cx,
                "slack-home-finder",
                "Channel or user name",
                false,
            ),
            dm_finder: new_slack_text_input(cx, "slack-dm-finder", "Find a direct message", false),
            new_message_to: new_slack_text_input(
                cx,
                "slack-new-message-to",
                "New message recipients",
                false,
            ),
            sidebar_section_name: new_slack_text_input(
                cx,
                "slack-sidebar-section-name",
                "Section name",
                false,
            ),
            later_reminder: new_slack_text_input(
                cx,
                "slack-later-reminder-description",
                "Reminder text",
                false,
            ),
        }
    }
}

struct SlackMessageInputs {
    forward_destination: SlackTextInput,
    forward_note: SlackTextInput,
    self_status_text: SlackTextInput,
    self_status_emoji: SlackTextInput,
    reaction_picker_search: SlackTextInput,
    composer: SlackTextInput,
    thread_composer: SlackTextInput,
    link_text: SlackTextInput,
    link_url: SlackTextInput,
}

impl SlackMessageInputs {
    fn new(cx: &mut Context<SurfaceState>) -> Self {
        Self {
            forward_destination: new_slack_text_input(
                cx,
                "slack-message-forward-destination",
                "Forward message destination",
                false,
            ),
            forward_note: new_slack_text_input(
                cx,
                "slack-message-forward-note",
                "Forward message note",
                true,
            ),
            self_status_text: new_slack_text_input(
                cx,
                "slack-self-status-text",
                "Status text",
                false,
            ),
            self_status_emoji: new_slack_text_input(
                cx,
                "slack-self-status-emoji",
                "Status emoji",
                false,
            ),
            reaction_picker_search: new_slack_text_input(
                cx,
                "slack-reaction-picker-search",
                "Emoji name",
                false,
            ),
            composer: new_slack_text_input(cx, "slack-message-composer", "Message composer", true),
            thread_composer: new_slack_text_input(
                cx,
                "slack-thread-reply-composer",
                "Thread reply composer",
                true,
            ),
            link_text: new_slack_text_input(cx, "slack-composer-link-text", "Text", false),
            link_url: new_slack_text_input(cx, "slack-composer-link-url", "Link", false),
        }
    }
}

struct SlackScheduleInputs {
    date: SlackTextInput,
    time: SlackTextInput,
}

impl SlackScheduleInputs {
    fn new(cx: &mut Context<SurfaceState>) -> Self {
        Self {
            date: new_slack_text_input(
                cx,
                "slack-schedule-date-input",
                "Scheduled message date",
                false,
            ),
            time: new_slack_text_input(
                cx,
                "slack-schedule-time-input",
                "Scheduled message time",
                false,
            ),
        }
    }
}

impl SlackSurfaceInputs {
    pub(super) fn new(cx: &mut Context<SurfaceState>) -> Self {
        let search = SlackSearchInputs::new(cx);
        let navigation = SlackNavigationInputs::new(cx);
        let message = SlackMessageInputs::new(cx);
        let schedule = SlackScheduleInputs::new(cx);
        Self {
            search: search.search,
            files_search: search.files,
            conversation_files_search: search.conversation_files,
            members_search: search.members,
            directory_search: navigation.directory_search,
            home_finder: navigation.home_finder,
            dm_finder: navigation.dm_finder,
            new_message_to: navigation.new_message_to,
            forward_destination: message.forward_destination,
            forward_note: message.forward_note,
            self_status_text: message.self_status_text,
            self_status_emoji: message.self_status_emoji,
            sidebar_section_name: navigation.sidebar_section_name,
            later_reminder: navigation.later_reminder,
            reaction_picker_search: message.reaction_picker_search,
            composer: message.composer,
            thread_composer: message.thread_composer,
            link_text: message.link_text,
            link_url: message.link_url,
            schedule_date: schedule.date,
            schedule_time: schedule.time,
        }
    }
}

fn new_slack_text_input(
    cx: &mut Context<SurfaceState>,
    element_name: &'static str,
    accessibility_label: &'static str,
    multiline: bool,
) -> SlackTextInput {
    let accessibility_id = ElementId::NamedInteger(element_name.into(), cx.entity_id().as_u64());
    let input_accessibility_id = accessibility_id.clone();
    let entity = cx.new(move |cx| {
        let props = if multiline {
            TextInputProps::multiline("")
        } else {
            TextInputProps::single_line("")
        };
        TextInput::new(
            props.accessibility(input_accessibility_id, accessibility_label),
            cx,
        )
    });
    SlackTextInput {
        entity,
        accessibility_id,
    }
}
