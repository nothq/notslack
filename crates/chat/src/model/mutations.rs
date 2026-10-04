#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackReactionMutation {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackStarMutation {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackSavedMessageMutation {
    Save,
    Remove,
}

impl SlackSavedMessageMutation {
    pub fn active_after(self) -> bool {
        matches!(self, Self::Save)
    }
}

impl SlackStarMutation {
    pub fn active_after(self) -> bool {
        matches!(self, Self::Add)
    }
}

impl SlackReactionMutation {
    pub fn active_after(self) -> bool {
        matches!(self, Self::Add)
    }
}
