use std::collections::HashSet;

const SLACK_SIDEBAR_SECTION_NAME_MAX_BYTES: usize = 80;
const SLACK_SIDEBAR_SECTION_CONVERSATION_LIMIT: usize = 1_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackSidebarSectionSort {
    #[default]
    Recent,
    Alphabetical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackSidebarSectionCreateRequest {
    name: String,
    conversation_ids: Vec<String>,
    sort: SlackSidebarSectionSort,
}

impl SlackSidebarSectionCreateRequest {
    pub fn new(
        name: String,
        conversation_ids: Vec<String>,
        sort: SlackSidebarSectionSort,
    ) -> Result<Self, String> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("Slack sidebar section name must not be empty".to_string());
        }
        if name.len() > SLACK_SIDEBAR_SECTION_NAME_MAX_BYTES {
            return Err(format!(
                "Slack sidebar section name exceeds {SLACK_SIDEBAR_SECTION_NAME_MAX_BYTES} bytes"
            ));
        }
        if !(1..=SLACK_SIDEBAR_SECTION_CONVERSATION_LIMIT).contains(&conversation_ids.len()) {
            return Err(format!(
                "Slack sidebar section requires between 1 and {SLACK_SIDEBAR_SECTION_CONVERSATION_LIMIT} conversations"
            ));
        }
        let mut unique_ids = HashSet::with_capacity(conversation_ids.len());
        for conversation_id in &conversation_ids {
            let mut characters = conversation_id.chars();
            let prefix = characters.next();
            if !matches!(prefix, Some('C' | 'D' | 'G'))
                || characters.clone().next().is_none()
                || !characters.all(|character| character.is_ascii_alphanumeric())
            {
                return Err(format!(
                    "invalid Slack sidebar section conversation id {conversation_id:?}"
                ));
            }
            if !unique_ids.insert(conversation_id.as_str()) {
                return Err(format!(
                    "Slack sidebar section contains duplicate conversation id {conversation_id}"
                ));
            }
        }
        Ok(Self {
            name,
            conversation_ids,
            sort,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn conversation_ids(&self) -> &[String] {
        &self.conversation_ids
    }

    pub fn sort(&self) -> SlackSidebarSectionSort {
        self.sort
    }
}
