use std::sync::Arc;

use gpui::Entity;

use super::{SlackConnectionApi, SurfaceState};
use crate::model::{
    ChatLaunchRoute, ChatTeamId, SlackWorkspaceDescriptor, SlackWorkspaceDirectory,
};

pub(super) struct SlackWorkspaceSlot {
    pub(super) descriptor: SlackWorkspaceDescriptor,
    pub(super) connection_generation: u64,
    pub(super) surface: Option<Entity<SurfaceState>>,
}

#[derive(Clone)]
pub(super) struct PendingSlackNotificationNavigation {
    pub(super) team_id: ChatTeamId,
    pub(super) selection_generation: u64,
    pub(super) conversation_id: String,
    pub(super) message_timestamp: Option<String>,
    pub(super) thread_timestamp: Option<String>,
    pub(super) launch_uri: Option<String>,
}

pub(super) struct SlackWorkspaceHost {
    pub(super) connection_api: Arc<dyn SlackConnectionApi>,
    pub(super) slots: Vec<SlackWorkspaceSlot>,
    pub(super) selected_team_id: ChatTeamId,
    pub(super) selection_generation: u64,
    pub(super) initial_route: Option<ChatLaunchRoute>,
}

impl SlackWorkspaceHost {
    pub(super) fn new(
        connection_api: Arc<dyn SlackConnectionApi>,
        initial_route: Option<ChatLaunchRoute>,
    ) -> Self {
        let directory = connection_api.workspace_directory();
        Self::from_directory(connection_api, directory, initial_route)
    }

    fn from_directory(
        connection_api: Arc<dyn SlackConnectionApi>,
        directory: SlackWorkspaceDirectory,
        initial_route: Option<ChatLaunchRoute>,
    ) -> Self {
        if initial_route
            .as_ref()
            .is_some_and(|route| route.team_id() != directory.initial_team_id())
        {
            panic!("Slack launch route must target the directory's initial workspace");
        }
        let selected_team_id = directory.initial_team_id().clone();
        let slots = directory
            .ordered()
            .iter()
            .cloned()
            .map(|descriptor| SlackWorkspaceSlot {
                descriptor,
                connection_generation: 0,
                surface: None,
            })
            .collect();
        Self {
            connection_api,
            slots,
            selected_team_id,
            selection_generation: 0,
            initial_route,
        }
    }

    pub(super) fn slot_index(&self, team_id: &ChatTeamId) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.descriptor.team_id == *team_id)
    }

    pub(super) fn selected_slot_index(&self) -> usize {
        self.slot_index(&self.selected_team_id)
            .expect("selected Slack workspace must remain in its ordered directory")
    }

    pub(super) fn select(&mut self, team_id: ChatTeamId) -> bool {
        assert!(
            self.slot_index(&team_id).is_some(),
            "selected Slack workspace must exist in its ordered directory"
        );
        if self.selected_team_id == team_id {
            return false;
        }
        self.selection_generation = self
            .selection_generation
            .checked_add(1)
            .expect("Slack workspace selection generation overflowed");
        self.selected_team_id = team_id;
        true
    }

    pub(super) fn selected_surface(&self) -> Option<Entity<SurfaceState>> {
        self.slots[self.selected_slot_index()].surface.clone()
    }

    pub(super) fn retained_surfaces(&self) -> impl Iterator<Item = Entity<SurfaceState>> + '_ {
        self.slots.iter().filter_map(|slot| slot.surface.clone())
    }
}
