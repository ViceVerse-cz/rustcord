use client_core::{Envelope, Event};
use model::Id;
use std::time::Instant;
fn main() {
    let start = Instant::now();
    let mut state = test_support::demo_state();
    let mut samples = Vec::new();
    for cycle in 0..200_u64 {
        for id in 1..=500 {
            state.apply(Envelope {
                generation: state.generation,
                event: Event::Message(test_support::message(1000 + cycle * 500 + id, Id(20))),
            });
        }
        samples.push(state.timeline.bytes());
        assert!(state.timeline.len() <= 500);
        assert!(state.timeline.bytes() <= 4 * 1024 * 1024);
    }
    let tail = &samples[100..];
    println!(
        "Synthetic reducer replay: 100000 events in {:?}; retained timeline {}..{} estimated bytes, {} records. This is not process RSS, UI frame time, or live compatibility.",
        start.elapsed(),
        tail.iter().min().unwrap(),
        tail.iter().max().unwrap(),
        state.timeline.len()
    );
    state.logout();
    assert_eq!(state.timeline.bytes(), 0);
    assert!(!state.has_unsent());
}
