use std::{collections::HashMap, sync::Arc};

use super::{PreparedSlackConversationSnapshot, PreparedSlackThreadSnapshot, SlackMessageRow};
use crate::ui::surface::SlackMessageActionTarget;
use crate::ui::{Image, SlackMessage, SlackReactionConversationScope};

mod lookup;
mod picker;
mod propagation;
mod receipt;
mod toggle;

pub(super) enum PreparedSlackReactionReceipt {
    Conversation {
        snapshot: Box<PreparedSlackConversationSnapshot>,
        scope: SlackReactionConversationScope,
    },
    Thread {
        snapshot: Box<PreparedSlackThreadSnapshot>,
        remote_images: HashMap<String, Arc<Image>>,
    },
}

pub(super) struct SlackAuthoritativeMessageInput<'a> {
    target: &'a SlackMessageActionTarget,
    message: &'a SlackMessage,
    row: &'a SlackMessageRow,
    remote_images: HashMap<String, Arc<Image>>,
}

impl<'a> SlackAuthoritativeMessageInput<'a> {
    pub(super) fn new(
        target: &'a SlackMessageActionTarget,
        message: &'a SlackMessage,
        row: &'a SlackMessageRow,
        remote_images: HashMap<String, Arc<Image>>,
    ) -> Self {
        Self {
            target,
            message,
            row,
            remote_images,
        }
    }
}
