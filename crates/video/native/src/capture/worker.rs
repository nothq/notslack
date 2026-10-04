use std::{
    path::PathBuf,
    sync::{
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
        Arc,
    },
    time::Duration,
};

use super::{
    platform, CaptureShared, NativeVideoCaptureError, NativeVideoCaptureMetadata,
    NativeVideoCaptureStatus,
};

const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) enum WorkerCommand {
    BeginRecording {
        scratch_movie_path: PathBuf,
        mp4_output_path: PathBuf,
        reply: SyncSender<Result<(), NativeVideoCaptureError>>,
    },
    Stop(SyncSender<Result<NativeVideoCaptureMetadata, NativeVideoCaptureError>>),
    Cancel(SyncSender<Result<(), NativeVideoCaptureError>>),
}

pub(super) fn run_worker(shared: Arc<CaptureShared>, commands: Receiver<WorkerCommand>) {
    let mut capture = match platform::PlatformVideoCapture::prepare(Arc::clone(&shared)) {
        Ok(capture) => capture,
        Err(error) => {
            shared.set_status(NativeVideoCaptureStatus::Failed(error.clone()));
            finish_failed_worker(error, commands);
            return;
        }
    };
    shared.update_native_status(capture.status());
    let mut mp4_output_path = None;
    loop {
        match commands.recv_timeout(STATUS_POLL_INTERVAL) {
            Ok(WorkerCommand::BeginRecording {
                scratch_movie_path,
                mp4_output_path: requested_output,
                reply,
            }) => {
                let result = begin_recording(
                    &mut capture,
                    &shared,
                    &mut mp4_output_path,
                    scratch_movie_path,
                    requested_output,
                );
                let _ = reply.send(result);
            }
            Ok(WorkerCommand::Stop(reply)) => {
                let result = stop_recording(&mut capture, mp4_output_path.take());
                let _ = reply.send(result);
                return;
            }
            Ok(WorkerCommand::Cancel(reply)) => {
                let _ = reply.send(capture.cancel());
                return;
            }
            Err(RecvTimeoutError::Timeout) => {
                shared.update_native_status(capture.status());
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = capture.cancel();
                return;
            }
        }
    }
}

fn begin_recording(
    capture: &mut platform::PlatformVideoCapture,
    shared: &CaptureShared,
    mp4_output_path: &mut Option<PathBuf>,
    scratch_movie_path: PathBuf,
    requested_output: PathBuf,
) -> Result<(), NativeVideoCaptureError> {
    if mp4_output_path.is_some() {
        return Err(NativeVideoCaptureError::AlreadyRecording);
    }
    shared.require_previewing()?;
    capture.begin_recording(&scratch_movie_path)?;
    *mp4_output_path = Some(requested_output);
    shared.update_native_status(capture.status());
    Ok(())
}

fn stop_recording(
    capture: &mut platform::PlatformVideoCapture,
    mp4_output_path: Option<PathBuf>,
) -> Result<NativeVideoCaptureMetadata, NativeVideoCaptureError> {
    let Some(output_path) = mp4_output_path else {
        let _ = capture.cancel();
        return Err(NativeVideoCaptureError::NotRecording);
    };
    capture.stop(&output_path)
}

fn finish_failed_worker(error: NativeVideoCaptureError, commands: Receiver<WorkerCommand>) {
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommand::BeginRecording { reply, .. } => {
                let _ = reply.send(Err(error.clone()));
            }
            WorkerCommand::Stop(reply) => {
                let _ = reply.send(Err(error));
                return;
            }
            WorkerCommand::Cancel(reply) => {
                let _ = reply.send(Ok(()));
                return;
            }
        }
    }
}
