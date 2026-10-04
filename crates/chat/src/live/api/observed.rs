use reqwest::blocking::Response;
use serde_json::Value;

use super::http::{slack_rate_limit_message, slack_retry_after_seconds};
use super::{SlackApiAuth, SlackApiClient, SlackObservedApiPost};

impl SlackApiClient {
    pub(crate) fn post_observed(
        &self,
        method: &str,
        params: &[(&str, String)],
    ) -> SlackObservedApiPost {
        if let Err(diagnostic) = self.record_call(method) {
            return SlackObservedApiPost::NotSent { diagnostic };
        }
        let response = match self.send_observed_request(method, params) {
            Ok(response) => response,
            Err(outcome) => return outcome,
        };
        observed_response(method, response)
    }

    fn send_observed_request(
        &self,
        method: &str,
        params: &[(&str, String)],
    ) -> Result<Response, SlackObservedApiPost> {
        let mut form = params.to_vec();
        let mut request = self.http.post(format!("{}/{}", self.base_url, method));
        if let SlackApiAuth::Desktop {
            xoxc_token,
            cookie_header,
            web_build_timestamp,
            ..
        } = &self.auth
        {
            form.insert(
                0,
                ("_x_version_ts", web_build_timestamp.as_str().to_string()),
            );
            form.insert(0, ("token", xoxc_token.clone()));
            request = request.header(reqwest::header::COOKIE, cookie_header.clone());
        } else {
            return Err(SlackObservedApiPost::NotSent {
                diagnostic: format!(
                    "Slack API request {method} requires authenticated Slack credentials"
                ),
            });
        }
        request
            .form(&form)
            .send()
            .map_err(|error| SlackObservedApiPost::Unknown {
                diagnostic: format!("Slack API request {method} outcome is unknown: {error}"),
            })
    }
}

fn observed_response(method: &str, response: Response) -> SlackObservedApiPost {
    if let Some(outcome) = observed_http_error(method, &response) {
        return outcome;
    }
    let body = match response.text() {
        Ok(body) => body,
        Err(error) => {
            return SlackObservedApiPost::Unknown {
                diagnostic: format!(
                    "Slack API response body for {method} could not be read: {error}"
                ),
            };
        }
    };
    let payload = match serde_json::from_str::<Value>(&body) {
        Ok(payload) => payload,
        Err(error) => {
            return SlackObservedApiPost::Unknown {
                diagnostic: format!(
                    "Slack API response for {method} could not be decoded: {error}"
                ),
            };
        }
    };
    observed_payload(method, payload)
}

fn observed_http_error(method: &str, response: &Response) -> Option<SlackObservedApiPost> {
    let status = response.status();
    let retry_after = slack_retry_after_seconds(response.headers());
    if status.as_u16() == 429 {
        Some(SlackObservedApiPost::Rejected {
            diagnostic: slack_rate_limit_message(method, retry_after),
        })
    } else if status.is_client_error() && status != reqwest::StatusCode::REQUEST_TIMEOUT {
        Some(SlackObservedApiPost::Rejected {
            diagnostic: format!("Slack API {method} returned HTTP {}", status.as_u16()),
        })
    } else if status.is_server_error() || !status.is_success() {
        Some(SlackObservedApiPost::Unknown {
            diagnostic: format!(
                "Slack API {method} outcome is unknown after HTTP {}",
                status.as_u16()
            ),
        })
    } else {
        None
    }
}

fn observed_payload(method: &str, payload: Value) -> SlackObservedApiPost {
    match payload.get("ok").and_then(Value::as_bool) {
        Some(true) => SlackObservedApiPost::Accepted(payload),
        Some(false) => {
            let error = payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown_error");
            SlackObservedApiPost::Rejected {
                diagnostic: format!("Slack API {method} failed: {error}"),
            }
        }
        None => SlackObservedApiPost::Unknown {
            diagnostic: format!(
                "Slack API {method} returned a malformed success response without ok=true"
            ),
        },
    }
}
