use std::num::NonZeroU64;

use super::{SlackAudioClipCaptureState, SlackMainComposerDraftHandle, SlackVideoClipCaptureState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackComposerCaptureGeneration(NonZeroU64);

impl SlackComposerCaptureGeneration {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackComposerCaptureOperation {
    pub(crate) generation: SlackComposerCaptureGeneration,
    pub(crate) owner: SlackMainComposerDraftHandle,
}

#[derive(Debug, Default)]
pub(crate) struct SlackComposerCaptureCoordinator {
    generation: u64,
    state: SlackComposerCaptureState,
}

#[derive(Debug, Default)]
pub(crate) enum SlackComposerCaptureState {
    #[default]
    Idle,
    Audio(SlackAudioClipCaptureState),
    Video(SlackVideoClipCaptureState),
}

impl SlackComposerCaptureCoordinator {
    pub(crate) fn next_operation(
        &mut self,
        owner: SlackMainComposerDraftHandle,
    ) -> SlackComposerCaptureOperation {
        assert!(
            self.can_start(),
            "a composer capture operation cannot replace an active capture"
        );
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Slack composer capture generation overflowed");
        SlackComposerCaptureOperation {
            generation: SlackComposerCaptureGeneration(
                NonZeroU64::new(self.generation)
                    .expect("Slack composer capture generation must be non-zero"),
            ),
            owner,
        }
    }

    pub(crate) fn state(&self) -> &SlackComposerCaptureState {
        &self.state
    }

    pub(crate) fn audio(&self) -> Option<&SlackAudioClipCaptureState> {
        match &self.state {
            SlackComposerCaptureState::Audio(state) => Some(state),
            SlackComposerCaptureState::Idle | SlackComposerCaptureState::Video(_) => None,
        }
    }

    pub(crate) fn audio_mut(&mut self) -> Option<&mut SlackAudioClipCaptureState> {
        match &mut self.state {
            SlackComposerCaptureState::Audio(state) => Some(state),
            SlackComposerCaptureState::Idle | SlackComposerCaptureState::Video(_) => None,
        }
    }

    pub(crate) fn video(&self) -> Option<&SlackVideoClipCaptureState> {
        match &self.state {
            SlackComposerCaptureState::Video(state) => Some(state),
            SlackComposerCaptureState::Idle | SlackComposerCaptureState::Audio(_) => None,
        }
    }

    pub(crate) fn set_audio(&mut self, state: SlackAudioClipCaptureState) {
        self.state = SlackComposerCaptureState::Audio(state);
    }

    pub(crate) fn set_video(&mut self, state: SlackVideoClipCaptureState) {
        self.state = SlackComposerCaptureState::Video(state);
    }

    pub(crate) fn set_idle(&mut self) {
        self.state = SlackComposerCaptureState::Idle;
    }

    pub(crate) fn can_start(&self) -> bool {
        match &self.state {
            SlackComposerCaptureState::Idle => true,
            SlackComposerCaptureState::Audio(state) => state.terminal(),
            SlackComposerCaptureState::Video(state) => state.terminal(),
        }
    }

    pub(crate) fn operation(&self) -> Option<&SlackComposerCaptureOperation> {
        match &self.state {
            SlackComposerCaptureState::Idle => None,
            SlackComposerCaptureState::Audio(state) => Some(state.operation()),
            SlackComposerCaptureState::Video(state) => Some(state.operation()),
        }
    }

    pub(crate) fn blocks_draft_submission(&self) -> bool {
        match &self.state {
            SlackComposerCaptureState::Idle => false,
            SlackComposerCaptureState::Audio(state) => state.blocks_draft_submission(),
            SlackComposerCaptureState::Video(state) => state.blocks_draft_submission(),
        }
    }
}
