use std::time::Duration;

use super::FfmpegVideoDecoder;

fn external_repro_source() -> Option<String> {
    std::env::var("NOTSLACK_VIDEO_NATIVE_REPRO_SOURCE").ok()
}

#[test]
fn external_video_source_decodes_frames() {
    let Some(source) = external_repro_source() else {
        return;
    };
    let mut decoder = FfmpegVideoDecoder::open(&source).expect("video decoder should open");
    let max_frames = std::env::var("NOTSLACK_VIDEO_NATIVE_REPRO_MAX_FRAMES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let mut decoded_frames = 0usize;
    while let Some(frame) = decoder.next_frame().expect("decode video frame") {
        assert!(!frame.frame.nv12_data.is_empty());
        decoded_frames += 1;
        if max_frames.is_some_and(|max_frames| decoded_frames >= max_frames) {
            break;
        }
    }
    assert!(decoded_frames > 0);
}

#[test]
fn external_video_source_decodes_after_seeks() {
    let Some(source) = external_repro_source() else {
        return;
    };
    let mut decoder = FfmpegVideoDecoder::open(&source).expect("video decoder should open");
    for target in [0, 1, 5, 30, 60, 120, 240, 295] {
        decoder
            .seek(Duration::from_secs(target))
            .expect("seek video decoder");
        for _ in 0..90 {
            let Some(frame) = decoder.next_frame().expect("decode video frame after seek") else {
                break;
            };
            assert!(!frame.frame.nv12_data.is_empty());
        }
    }
}

#[test]
fn external_video_source_survives_decoder_churn() {
    let Some(source) = external_repro_source() else {
        return;
    };
    let iterations = std::env::var("NOTSLACK_VIDEO_NATIVE_REPRO_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);
    for iteration in 0..iterations {
        let mut decoder = FfmpegVideoDecoder::open(&source).expect("video decoder should open");
        if iteration % 3 == 0 {
            decoder
                .seek(Duration::from_secs((iteration % 240) as u64))
                .expect("seek video decoder");
        }
        for _ in 0..5 {
            let Some(frame) = decoder.next_frame().expect("decode video frame") else {
                break;
            };
            assert!(!frame.frame.nv12_data.is_empty());
        }
    }
}
