use super::{beginning, SlackWorkspaceRuntime};

use crate::model::{
    SlackActivityCursor, SlackActivitySnapshot, SlackAllThreadsCursor, SlackAllThreadsSnapshot,
    SlackBookmarkFolderRequest, SlackBookmarkFolderSnapshot, SlackChannelDetailsSnapshot,
    SlackChannelNotificationMutation, SlackChannelNotificationPreference,
    SlackConversationFilesRequest, SlackConversationFilesSnapshot, SlackConversationHistoryCursor,
    SlackConversationHistoryPage, SlackConversationMembersCursor, SlackConversationMembersSnapshot,
    SlackConversationOpenReceipt, SlackConversationOpenRequest, SlackConversationReadReceipt,
    SlackConversationSnapshot, SlackDmInboxSnapshot, SlackDraftsSentRequest,
    SlackDraftsSentSnapshot, SlackFilesRequest, SlackFilesSnapshot, SlackLaterCursor,
    SlackLaterFilter, SlackLaterHydratedItem, SlackLaterHydrationTarget, SlackLaterSnapshot,
    SlackMessageDraft, SlackMessageForwardReceipt, SlackMessageSendReceipt, SlackPinsRequest,
    SlackPinsSnapshot, SlackProfile, SlackQuickSearchSnapshot, SlackReactionCatalogSnapshot,
    SlackScheduledDraftReceipt, SlackSearchSnapshot, SlackShellSnapshot, SlackSidebarSnapshot,
    SlackThreadLoad, SlackThreadReadTarget, SlackThreadReplyReceipt, SlackThreadSnapshot,
    SlackUploadFile, SlackWorkspace,
};
use remote_image_model::RemoteImageData;

mod mutations;
mod reads;
mod writes;

impl crate::model::SlackWorkspaceApi for SlackWorkspaceRuntime {
    reads::impl_workspace_api_reads!();
    writes::impl_workspace_api_writes!();
}
