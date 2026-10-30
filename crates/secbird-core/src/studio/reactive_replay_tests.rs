use super::*;
const A: LedAddress = LedAddress { row: 2, col: 3 };
const B: LedAddress = LedAddress { row: 2, col: 4 };
const C: LedAddress = LedAddress { row: 2, col: 5 };
fn key(code: u16) -> KeyboardKey {
    KeyboardKey::new(code, 0).unwrap()
}
fn input() -> ReactiveInputMap {
    ReactiveInputMap::from_device_json(
        r#"{"id":"keyboard","leds":[
        {"row":2,"col":3,"inputType":"kbd","inputData":"[30,0]"},
        {"row":2,"col":4,"inputType":"kbd","inputData":"[48,0]"},
        {"row":2,"col":5,"inputType":"kbd","inputData":"[46,0]"}]}"#,
    )
    .unwrap()
}
fn fixture(policy: Retrigger) -> ReactiveReplay {
    // Deliberately synthetic steps. These are NOT a reference vendor fade.
    ReactiveReplay::new(
        input(),
        &[A, B, C],
        vec![ReplayRegion {
            leds: vec![A, B],
            pulse: PulseRecording::new(
                40,
                &[
                    Pixel::opaque(Rgb(200, 0, 0)),
                    Pixel::new(Rgb(100, 0, 0), 50).unwrap(),
                    Pixel::EMPTY,
                ],
            )
            .unwrap(),
        }],
        policy,
    )
    .unwrap()
}
fn frame(
    replay: &mut ReactiveReplay,
    session: &ReplaySession,
    ms: u64,
    brightness: u8,
) -> Vec<Rgb> {
    let mut output = vec![Rgb(0, 0, 100); 3];
    assert!(replay.blend(session, ms, brightness, &mut output).unwrap());
    output
}
#[test]
fn preview_opt_in_and_foreign_or_stale_sessions_are_inert() {
    let mut r = fixture(Retrigger::Restart);
    let mut other = fixture(Retrigger::Restart);
    let foreign = other.start_local_preview();
    assert_eq!(r.key(&foreign, "keyboard", key(30), true, 0).unwrap(), 0);
    let mut output = vec![Rgb(1, 2, 3); 3];
    assert!(!r.blend(&foreign, 0, 100, &mut output).unwrap());
    assert_eq!(output, vec![Rgb(1, 2, 3); 3]);
    let old = r.start_local_preview();
    let current = r.start_local_preview();
    assert_eq!(r.key(&old, "keyboard", key(30), true, 0).unwrap(), 0);
    assert_eq!(r.key(&current, "foreign", key(30), true, 0).unwrap(), 0);
    assert_eq!(frame(&mut r, &current, 0, 100), vec![Rgb(0, 0, 100); 3]);
}
#[test]
fn sample_boundaries_and_overlapping_keys_are_independent_and_region_filtered() {
    let mut r = fixture(Retrigger::Restart);
    let s = r.start_local_preview();
    assert_eq!(r.key(&s, "keyboard", key(30), true, 100).unwrap(), 1);
    assert_eq!(
        frame(&mut r, &s, 100, 100),
        [Rgb(200, 0, 0), Rgb(0, 0, 100), Rgb(0, 0, 100)]
    );
    assert_eq!(frame(&mut r, &s, 139, 100)[0], Rgb(200, 0, 0));
    assert_eq!(r.key(&s, "keyboard", key(48), true, 140).unwrap(), 1);
    assert_eq!(r.key(&s, "keyboard", key(46), true, 140).unwrap(), 0);
    assert_eq!(
        frame(&mut r, &s, 140, 100),
        [Rgb(50, 0, 50), Rgb(200, 0, 0), Rgb(0, 0, 100)]
    );
    assert_eq!(
        frame(&mut r, &s, 180, 100),
        [Rgb(0, 0, 100), Rgb(50, 0, 50), Rgb(0, 0, 100)]
    );
    assert_eq!(frame(&mut r, &s, 220, 100), vec![Rgb(0, 0, 100); 3]);
    assert_eq!(frame(&mut r, &s, 260, 100), vec![Rgb(0, 0, 100); 3]);
    assert!(r.regions[0]
        .active
        .iter()
        .all(|(_, active)| active.is_none()));
}
#[test]
fn repeat_is_bounded_and_retrigger_policy_is_explicit() {
    for (policy, expected) in [(Retrigger::Restart, 1), (Retrigger::IgnoreWhileActive, 0)] {
        let mut r = fixture(policy);
        let s = r.start_local_preview();
        assert_eq!(r.key(&s, "keyboard", key(30), true, 0).unwrap(), 1);
        for _ in 0..10000 {
            assert_eq!(r.key(&s, "keyboard", key(30), true, 0).unwrap(), 0);
        }
        assert_eq!(r.held.len(), 1);
        assert_eq!(r.regions[0].active.len(), 2);
        r.key(&s, "keyboard", key(30), false, 40).unwrap();
        assert_eq!(r.key(&s, "keyboard", key(30), true, 40).unwrap(), expected);
        r.key(&s, "keyboard", key(30), false, 120).unwrap();
        assert_eq!(r.key(&s, "keyboard", key(30), true, 200).unwrap(), 1);
    }
}
#[test]
fn none_and_black_differ_and_brightness_runs_after_layer_composition() {
    let mut r = ReactiveReplay::new(
        input(),
        &[A, B, C],
        vec![
            ReplayRegion {
                leds: vec![A],
                pulse: PulseRecording::new(40, &[Pixel::opaque(Rgb(200, 100, 0))]).unwrap(),
            },
            ReplayRegion {
                leds: vec![A, B],
                pulse: PulseRecording::new(40, &[Pixel::EMPTY, Pixel::opaque(Rgb(0, 0, 0))])
                    .unwrap(),
            },
        ],
        Retrigger::Restart,
    )
    .unwrap();
    let s = r.start_local_preview();
    r.key(&s, "keyboard", key(30), true, 0).unwrap();
    r.key(&s, "keyboard", key(48), true, 0).unwrap();
    assert_eq!(
        frame(&mut r, &s, 0, 50),
        [Rgb(100, 50, 0), Rgb(0, 0, 50), Rgb(0, 0, 50)]
    );
    assert_eq!(
        frame(&mut r, &s, 40, 100),
        [Rgb(0, 0, 0), Rgb(0, 0, 0), Rgb(0, 0, 100)]
    );
    assert_eq!(frame(&mut r, &s, 80, 0), vec![Rgb(0, 0, 0); 3]);
}
#[test]
fn stop_unplug_error_and_restart_clear_every_held_key_and_pulse() {
    let mut r = fixture(Retrigger::Restart);
    let old = r.start_local_preview();
    r.key(&old, "keyboard", key(30), true, 100).unwrap();
    r.stop();
    assert!(r.held.is_empty());
    assert!(r.regions[0].active.iter().all(|(_, at)| at.is_none()));
    let new = r.start_local_preview();
    assert_eq!(frame(&mut r, &new, 0, 100), vec![Rgb(0, 0, 100); 3]);
    assert_eq!(r.key(&old, "keyboard", key(30), true, 100).unwrap(), 0);
    assert_eq!(r.key(&new, "keyboard", key(30), true, 100).unwrap(), 1);
    assert!(r.blend(&new, 99, 100, &mut [Rgb(0, 0, 0); 3]).is_err());
    assert!(r.session.is_none());
}
#[test]
fn invalid_frames_do_not_consume_time_or_mutate_caller_output() {
    let mut r = fixture(Retrigger::Restart);
    let s = r.start_local_preview();
    r.key(&s, "keyboard", key(30), true, 0).unwrap();
    let mut output = [Rgb(1, 2, 3); 2];
    assert!(r.blend(&s, 500, 100, &mut output).is_err());
    assert_eq!(output, [Rgb(1, 2, 3); 2]);
    assert_eq!(frame(&mut r, &s, 0, 100)[0], Rgb(200, 0, 0));
}
#[test]
fn malformed_regions_and_sample_budgets_refuse_before_playback() {
    assert!(PulseRecording::new(0, &[Pixel::EMPTY]).is_err());
    assert!(PulseRecording::new(40, &[]).is_err());
    assert!(PulseRecording::new(40, &vec![Pixel::EMPTY; 16385]).is_err());
    for leds in [vec![A, A], vec![LedAddress { row: 6, col: 22 }]] {
        assert!(ReactiveReplay::new(
            input(),
            &[A, B, C],
            vec![ReplayRegion {
                leds,
                pulse: PulseRecording::new(40, &[Pixel::EMPTY]).unwrap()
            }],
            Retrigger::Restart
        )
        .is_err());
    }
    assert!(ReactiveReplay::new(input(), &[A, A], vec![], Retrigger::Restart).is_err());
    let regions = (0..65)
        .map(|_| ReplayRegion {
            leds: vec![A],
            pulse: PulseRecording::new(40, &vec![Pixel::EMPTY; 16384]).unwrap(),
        })
        .collect();
    assert!(ReactiveReplay::new(input(), &[A, B, C], regions, Retrigger::Restart).is_err());
}

#[test]
fn diagnostic_samples_do_not_admit_unverified_reactive_to_production_preview() {
    let mut raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/studio-preview-profile.json"
    ))
    .unwrap();
    raw["layers"][0]["name"] = "reactive".into();
    raw["layers"][0]["value"] = 3.into();
    let draft = crate::studio::document::Draft::open(&raw.to_string()).unwrap();
    let original = draft.json().to_owned();
    let result = crate::studio::document::Preview::from_snapshot(&draft.read_snapshot(), 1);
    assert!(
        matches!(result,Err(error) if error.to_string().contains("preview does not yet implement reactive"))
    );
    assert_eq!(draft.json(), original);
    assert!(!draft.is_dirty());
}
