mod decode;
mod validation;

use crate::model::{SlackPinsRequest, SlackPinsSnapshot};

use super::SlackLiveWorkspaceLoader;
use decode::SlackPinsListResponse;

const PINS_LIST: &str = "pins.list";

impl SlackLiveWorkspaceLoader {
    pub fn load_pins(&self, request: SlackPinsRequest) -> Result<SlackPinsSnapshot, String> {
        if request.team_id != self.team_id {
            return Err(format!(
                "Slack Pins request targeted team {} from runtime team {}",
                request.team_id, self.team_id
            ));
        }
        let payload = self
            .api
            .post(PINS_LIST, &[("channel", request.conversation_id.clone())])?;
        let response = serde_json::from_value::<SlackPinsListResponse>(payload)
            .map_err(|error| format!("failed to decode Slack {PINS_LIST} response: {error}"))?;
        self.decode_pins_response(request, response)
    }
}
