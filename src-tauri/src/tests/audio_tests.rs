// HalluMeter

use crate::audio::AudioPlayer;
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

#[test]
fn mute_flag_starts_false() {
    let player = AudioPlayer::new();
    assert!(!player.muted.load(Ordering::Relaxed));
}

#[test]
fn mute_flag_set_true() {
    let player = AudioPlayer::new();
    player.muted.store(true, Ordering::Relaxed);
    assert!(player.muted.load(Ordering::Relaxed));
}

#[test]
fn mute_flag_shared_via_arc() {
    let player = AudioPlayer::new();
    let shared = player.muted.clone();
    shared.store(true, Ordering::Relaxed);
    assert!(player.muted.load(Ordering::Relaxed));
}

#[test]
fn mute_flag_toggle() {
    let player = AudioPlayer::new();
    let shared = player.muted.clone();
    shared.store(true, Ordering::Relaxed);
    assert!(player.muted.load(Ordering::Relaxed));
    shared.store(false, Ordering::Relaxed);
    assert!(!player.muted.load(Ordering::Relaxed));
}

#[test]
fn rand_1_5_hits_all_variants_with_reasonable_distribution() {
    let mut player = AudioPlayer::new();
    let mut counts: BTreeMap<u8, usize> = (1..=5).map(|variant| (variant, 0)).collect();

    for _ in 0..10_000 {
        let variant = player.rand_1_5();
        assert!(
            (1..=5).contains(&variant),
            "variant out of range: {variant}"
        );
        *counts.get_mut(&variant).expect("known variant bucket") += 1;
    }

    for variant in 1..=5 {
        let count = counts[&variant];
        assert!(count > 0, "variant {variant} was never selected");
        assert!(
            (1700..=2300).contains(&count),
            "variant {variant} count {count} looks too skewed; counts={counts:?}"
        );
    }
}

#[test]
fn rand_1_5_avoiding_prevents_immediate_repeat() {
    let mut player = AudioPlayer::new();

    for last in 1..=5 {
        for _ in 0..200 {
            let variant = player.rand_1_5_avoiding(Some(last));
            assert!(
                (1..=5).contains(&variant),
                "variant out of range: {variant}"
            );
            assert_ne!(variant, last, "variant repeated immediately: {variant}");
        }
    }
}

#[test]
fn rand_threshold_stays_in_new_range() {
    let mut player = AudioPlayer::new();

    for _ in 0..1_000 {
        let threshold = player.rand_threshold();
        assert!(
            (8..=14).contains(&threshold),
            "threshold out of range: {threshold}"
        );
    }
}
