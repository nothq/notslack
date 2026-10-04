use crate::ui::SlackSkinTone;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackReactionSkinToneSupport {
    None,
    Single,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSkinToneLoadRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackSkinToneMutationRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) previous: Option<SlackSkinTone>,
    pub(crate) selected: SlackSkinTone,
}
