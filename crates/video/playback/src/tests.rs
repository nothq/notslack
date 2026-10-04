use crate::{
    prepare_video_source, session::VideoSession, video_clock_label, video_source_kind,
    VideoPlayerSource, VideoSourceKind,
};
use std::time::Duration;

#[test]
fn video_clock_labels_short_and_long_durations() {
    assert_eq!(video_clock_label(0.0), "0:00");
    assert_eq!(video_clock_label(75.4), "1:15");
    assert_eq!(video_clock_label(3671.0), "1:01:11");
}

#[test]
fn source_kind_parses_supported_formats_once() {
    assert_eq!(
        video_source_kind("https://example.test/video.webm?download=1"),
        VideoSourceKind::Native
    );
    assert_eq!(
        video_source_kind("https://example.test/video.mp4?format=webm"),
        VideoSourceKind::Native
    );
    assert_eq!(
        video_source_kind("https://example.test/video.bin.zst"),
        VideoSourceKind::Unsupported("Zstandard telemetry archive")
    );
    assert_eq!(
        video_source_kind("https://example.test/video.ogv"),
        VideoSourceKind::Native
    );
}

#[test]
fn local_webm_sources_remain_direct_native_sources() {
    let source = prepare_video_source("/tmp/robot-log.webm").expect("local webm path should parse");

    assert_eq!(source.uri().scheme(), "file");
    assert!(source.uri().path().ends_with("/tmp/robot-log.webm"));
}

#[test]
fn source_constructor_preserves_display_fields() {
    let source = VideoPlayerSource::new(
        "chunk-1",
        "10:31",
        "1x",
        "https://example.test/a.mp4",
        Some(631),
    );

    assert_eq!(source.id.as_ref(), "chunk-1");
    assert_eq!(source.title.as_ref(), "10:31");
    assert_eq!(source.subtitle.as_ref(), "1x");
    assert_eq!(source.url, "https://example.test/a.mp4");
    assert_eq!(source.start_seconds, Some(631));
}

#[test]
fn external_video_source_survives_session_churn() {
    let Ok(source) = std::env::var("NOTSLACK_VIDEO_PLAYBACK_REPRO_SOURCE") else {
        return;
    };
    let iterations = std::env::var("NOTSLACK_VIDEO_PLAYBACK_REPRO_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(50);
    for iteration in 0..iterations {
        let mut session = VideoSession::new(source.as_str()).expect("video session should open");
        session.sync();
        if iteration % 2 == 0 {
            session.seek_to_fraction(0.75);
        }
        if iteration % 3 == 0 {
            session.pause();
        }
    }
    std::thread::sleep(Duration::from_secs(2));
}
