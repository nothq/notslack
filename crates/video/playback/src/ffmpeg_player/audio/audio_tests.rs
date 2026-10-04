use super::output::{scale_sample, AudioSampleQueue};

#[test]
fn queue_outputs_silence_on_underrun() {
    let queue = AudioSampleQueue::new(2);

    assert_eq!(queue.pop_output_sample(false, 1.0), 0);
}

#[test]
fn queue_drops_oldest_samples_on_overflow() {
    let queue = AudioSampleQueue::new(3);

    queue.push_interleaved(&[1, 2, 3, 4, 5]);

    assert_eq!(queue.pop_output_sample(false, 1.0), 3);
    assert_eq!(queue.pop_output_sample(false, 1.0), 4);
    assert_eq!(queue.pop_output_sample(false, 1.0), 5);
}

#[test]
fn mute_drains_without_replaying_old_samples() {
    let queue = AudioSampleQueue::new(3);
    queue.push_interleaved(&[100, 200]);

    assert_eq!(queue.pop_output_sample(true, 1.0), 0);
    assert_eq!(queue.pop_output_sample(false, 1.0), 200);
}

#[test]
fn volume_scales_and_clamps_samples() {
    assert_eq!(scale_sample(1000, 0.5), 500);
    assert_eq!(scale_sample(i16::MAX, 2.0), i16::MAX);
    assert_eq!(scale_sample(i16::MIN, 2.0), i16::MIN);
}
