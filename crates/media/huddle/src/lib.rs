//! Huddle audio: joins the Amazon Chime meeting behind a Slack huddle and
//! carries the microphone and speaker over WebRTC, the way the Amazon Chime
//! SDK for JavaScript does inside Slack's own client.
//!
//! Slack's `rooms.join` hands out the meeting's signaling URL, audio host and
//! the attendee's join token; [`HuddleSession::start`] takes it from there.

mod audio;
mod protocol;
mod session;
mod signaling;

use std::thread::{self, JoinHandle};

use tokio::sync::mpsc;

/// What the huddle connects with, from Slack's `rooms.join`.
#[derive(Clone, Debug)]
pub struct HuddleMediaConfig {
    /// `MediaPlacement.SignalingUrl`.
    pub signaling_url: String,
    /// `MediaPlacement.AudioHostUrl`.
    pub audio_host_url: String,
    /// `Attendee.AttendeeId`.
    pub attendee_id: String,
    /// `Attendee.JoinToken`.
    pub join_token: String,
    /// Starts the huddle muted.
    pub muted: bool,
}

/// What the huddle reports while it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HuddleEvent {
    /// Audio is flowing both ways.
    Connected,
    /// Whether the microphone is muted; also sent when there is no
    /// microphone, which keeps the huddle muted.
    Muted(bool),
    /// The people in the huddle, by the external user id Slack gave Chime
    /// (the Slack user id, when Slack sets it that way).
    Participants(Vec<String>),
    /// The huddle ended after [`HuddleSession::leave`] or from the other end.
    Ended,
    /// The huddle could not connect or broke off.
    Failed(String),
}

enum HuddleCommand {
    SetMuted(bool),
    Leave,
}

/// A running huddle connection. Dropping it leaves the huddle.
pub struct HuddleSession {
    commands: mpsc::UnboundedSender<HuddleCommand>,
    thread: Option<JoinHandle<()>>,
}

impl HuddleSession {
    /// Connects in the background; progress arrives on `events`.
    pub fn start(
        config: HuddleMediaConfig,
        events: async_channel::Sender<HuddleEvent>,
    ) -> Result<Self, String> {
        let (commands, command_receiver) = mpsc::unbounded_channel();
        let thread = thread::Builder::new()
            .name("notslack-huddle".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = events.try_send(HuddleEvent::Failed(format!(
                            "could not start the huddle runtime: {error}"
                        )));
                        return;
                    }
                };
                runtime.block_on(session::run(config, command_receiver, events));
            })
            .map_err(|error| format!("could not start the huddle thread: {error}"))?;
        Ok(Self {
            commands,
            thread: Some(thread),
        })
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.commands.send(HuddleCommand::SetMuted(muted));
    }

    /// Leaves the huddle; [`HuddleEvent::Ended`] follows.
    pub fn leave(&self) {
        let _ = self.commands.send(HuddleCommand::Leave);
    }
}

impl Drop for HuddleSession {
    fn drop(&mut self) {
        let _ = self.commands.send(HuddleCommand::Leave);
        // The session says goodbye to Chime with a short timeout, so this
        // wait stays brief.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
