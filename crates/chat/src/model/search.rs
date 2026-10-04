#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlackMessageSearchSort {
    #[default]
    Relevant,
    Recent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlackMessageSearchOptions {
    pub from: Option<String>,
    pub in_conversation: Option<String>,
    pub only_my_channels: bool,
    pub include_automations: bool,
    pub sort: SlackMessageSearchSort,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackMessageSearchRequest {
    pub query: String,
    pub cursor: Option<String>,
    pub options: SlackMessageSearchOptions,
}
