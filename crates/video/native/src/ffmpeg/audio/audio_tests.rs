use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use super::{probe_audio, FfmpegAudioDecoder, FfmpegAudioOutputConfig, FfmpegError};

#[test]
fn audio_decoder_outputs_nonzero_pcm_samples() {
    let fixture = TestMediaDir::new("audio-decoder");
    let audio_path = fixture.path("sine.wav");
    generate_sine_wav(&audio_path, "0.25");

    probe_audio(path_str(&audio_path)).expect("audio stream should be discoverable");
    let mut decoder = FfmpegAudioDecoder::open(
        path_str(&audio_path),
        FfmpegAudioOutputConfig {
            sample_rate: 48_000,
            channels: 2,
        },
    )
    .expect("audio decoder should open");
    let mut samples = Vec::new();
    while samples.len() < 2048 {
        let Some(frame) = decoder.next_frame().expect("decode audio frame") else {
            break;
        };
        samples.extend(frame.samples);
    }

    assert!(samples.iter().any(|sample| *sample != 0));
}

#[test]
fn audio_decoder_seek_continues_decoding_pcm() {
    let fixture = TestMediaDir::new("audio-seek");
    let audio_path = fixture.path("sine.wav");
    generate_sine_wav(&audio_path, "1.0");

    let mut decoder = FfmpegAudioDecoder::open(
        path_str(&audio_path),
        FfmpegAudioOutputConfig {
            sample_rate: 48_000,
            channels: 1,
        },
    )
    .expect("audio decoder should open");
    decoder
        .seek(Duration::from_millis(500))
        .expect("seek audio decoder");
    let frame = decoder
        .next_frame()
        .expect("decode after seek")
        .expect("audio frame after seek");

    assert!(frame.samples.iter().any(|sample| *sample != 0));
}

#[test]
fn video_only_media_reports_missing_audio_stream() {
    let fixture = TestMediaDir::new("video-only");
    let video_path = fixture.path("video-only.mp4");
    generate_video_only_mp4(&video_path);

    assert!(matches!(
        probe_audio(path_str(&video_path)),
        Err(FfmpegError::AudioStream)
    ));
}

struct TestMediaDir {
    root: PathBuf,
}

impl TestMediaDir {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("notslack-video-native-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create media fixture dir");
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TestMediaDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn generate_sine_wav(path: &Path, duration: &str) {
    run_ffmpeg(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={duration}"),
                "-c:a",
                "pcm_s16le",
            ])
            .arg(path),
    );
}

fn generate_video_only_mp4(path: &Path) {
    run_ffmpeg(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=0.25:size=16x16:rate=10",
                "-an",
                "-c:v",
                "mpeg4",
            ])
            .arg(path),
    );
}

fn run_ffmpeg(command: &mut Command) {
    let status = command.status().expect("run ffmpeg fixture generation");
    assert!(status.success(), "ffmpeg fixture generation failed");
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("fixture path should be UTF-8")
}
