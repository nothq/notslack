use std::{
    io::{Error as IoError, ErrorKind, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::model::{SlackUploadFile, SlackUploadReader};

use super::http::{get_bytes_with_request, slack_authenticated_https_url, slack_url_requires_auth};
use super::{
    SlackApiAuth, SlackApiClient, SlackBytesResponse, SlackMediaRequestMethod, SlackObservedUpload,
    MAX_SLACK_REMOTE_IMAGE_BYTES,
};

impl SlackApiClient {
    pub(crate) fn get_remote_image_bytes(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<SlackBytesResponse, String> {
        self.get_bytes_with_limit(url, timeout, MAX_SLACK_REMOTE_IMAGE_BYTES)
    }

    fn get_bytes_with_limit(
        &self,
        url: &str,
        timeout: Duration,
        max_bytes: usize,
    ) -> Result<SlackBytesResponse, String> {
        let mut request = self.http.get(url).timeout(timeout);
        if slack_url_requires_auth(url) {
            match &self.auth {
                SlackApiAuth::Desktop { cookie_header, .. } => {
                    request = request.header(reqwest::header::COOKIE, cookie_header.clone());
                }
                SlackApiAuth::Public => {
                    return Err(format!(
                        "Slack file request requires authenticated Slack credentials for {url}"
                    ));
                }
            }
        }
        get_bytes_with_request(request, url, max_bytes)
    }

    pub(crate) async fn open_media(
        &self,
        url: &str,
        method: SlackMediaRequestMethod,
        range: Option<&str>,
    ) -> Result<reqwest::Response, String> {
        if !reqwest::Url::parse(url).is_ok_and(|url| slack_authenticated_https_url(&url)) {
            return Err(
                "Slack media source must use an authenticated Slack HTTPS host".to_string(),
            );
        }
        let SlackApiAuth::Desktop {
            cookie_header,
            media_http,
            ..
        } = &self.auth
        else {
            return Err(format!(
                "Slack media request requires authenticated Slack credentials for {url}"
            ));
        };
        let mut request = match method {
            SlackMediaRequestMethod::Get => media_http.get(url),
            SlackMediaRequestMethod::Head => media_http.head(url),
        }
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header(reqwest::header::COOKIE, cookie_header.clone());
        if let Some(range) = range {
            request = request.header(reqwest::header::RANGE, range);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Slack media request failed for {url}: {error}"))?;
        if !slack_authenticated_https_url(response.url()) {
            return Err(format!(
                "Slack media request redirected outside authenticated Slack HTTPS hosts to {}",
                response.url()
            ));
        }
        Ok(response)
    }

    pub(crate) fn upload_file(&self, url: &str, file: &SlackUploadFile) -> Result<(), String> {
        let upload_url = reqwest::Url::parse(url)
            .map_err(|error| format!("Slack upload URL is invalid: {error}"))?;
        if !slack_authenticated_https_url(&upload_url) {
            return Err("Slack upload URL must use an authenticated Slack HTTPS host".to_string());
        }
        let reader = file.open_reader()?;
        let response = self
            .http
            .post(upload_url)
            .header(reqwest::header::CONTENT_TYPE, file.mimetype())
            .body(reqwest::blocking::Body::sized(reader, file.size_bytes()))
            .send()
            .map_err(|error| format!("Slack upload request failed: {error}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "Slack upload request returned HTTP {}",
                status.as_u16()
            ));
        }
        Ok(())
    }

    pub(crate) fn upload_file_observed(
        &self,
        url: &str,
        file: &SlackUploadFile,
        cancellation: Arc<AtomicBool>,
    ) -> SlackObservedUpload {
        if cancellation.load(Ordering::Acquire) {
            return SlackObservedUpload::Cancelled;
        }
        let upload_url = match observed_upload_url(url) {
            Ok(upload_url) => upload_url,
            Err(outcome) => return outcome,
        };
        let reader = match file.open_reader() {
            Ok(reader) => reader,
            Err(error) => {
                return SlackObservedUpload::Rejected { diagnostic: error };
            }
        };
        let body = cancellable_upload_body(reader, file.size_bytes(), &cancellation);
        let response = self
            .http
            .post(upload_url)
            .header(reqwest::header::CONTENT_TYPE, file.mimetype())
            .body(body)
            .send();
        if cancellation.load(Ordering::Acquire) {
            return SlackObservedUpload::Cancelled;
        }
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return SlackObservedUpload::Unknown {
                    diagnostic: format!("Slack upload request outcome is unknown: {error}"),
                };
            }
        };
        if !response.status().is_success() {
            return SlackObservedUpload::Rejected {
                diagnostic: format!(
                    "Slack upload request returned HTTP {}",
                    response.status().as_u16()
                ),
            };
        }
        SlackObservedUpload::Transferred
    }
}

fn observed_upload_url(url: &str) -> Result<reqwest::Url, SlackObservedUpload> {
    let upload_url = reqwest::Url::parse(url).map_err(|error| SlackObservedUpload::Rejected {
        diagnostic: format!("Slack upload URL is invalid: {error}"),
    })?;
    if !slack_authenticated_https_url(&upload_url) {
        return Err(SlackObservedUpload::Rejected {
            diagnostic: "Slack upload URL must use an authenticated Slack HTTPS host".to_string(),
        });
    }
    Ok(upload_url)
}

fn cancellable_upload_body(
    reader: SlackUploadReader,
    size_bytes: u64,
    cancellation: &Arc<AtomicBool>,
) -> reqwest::blocking::Body {
    reqwest::blocking::Body::sized(
        CancellableUploadReader {
            reader,
            cancellation: Arc::clone(cancellation),
        },
        size_bytes,
    )
}

struct CancellableUploadReader {
    reader: SlackUploadReader,
    cancellation: Arc<AtomicBool>,
}

impl Read for CancellableUploadReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(IoError::new(
                ErrorKind::Interrupted,
                "Slack upload cancelled",
            ));
        }
        let read = self.reader.read(buffer)?;
        if self.cancellation.load(Ordering::Acquire) {
            return Err(IoError::new(
                ErrorKind::Interrupted,
                "Slack upload cancelled",
            ));
        }
        Ok(read)
    }
}
