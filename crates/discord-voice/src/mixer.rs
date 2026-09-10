//! One bounded decoder and reorder window per authenticated remote speaker.
use crate::{Frame, crypto::MAX_PARTICIPANTS, jitter::Jitter};
use opus2::{Channels, Decoder};

struct Speaker {
    user: u64,
    ssrc: u32,
    decoder: Decoder,
    jitter: Jitter,
    pcm: Box<[f32; 5760]>,
    offset: usize,
    length: usize,
}
#[derive(Default)]
pub(crate) struct Mixer {
    speakers: Vec<Speaker>,
}
impl Mixer {
    pub fn announce(&mut self, user: u64, ssrc: u32) -> Result<(), &'static str> {
        if self
            .speakers
            .iter()
            .any(|s| s.ssrc == ssrc && s.user != user)
        {
            return Err("Voice SSRC belongs to another participant");
        }
        if self
            .speakers
            .iter()
            .any(|s| s.user == user && s.ssrc == ssrc)
        {
            return Ok(());
        }
        self.remove(user);
        if self.speakers.len() >= MAX_PARTICIPANTS - 1 {
            return Err("Voice decoder budget exceeded");
        }
        self.speakers.push(Speaker {
            user,
            ssrc,
            decoder: Decoder::new(48_000, Channels::Mono)
                .map_err(|_| "Opus decoder initialization failed")?,
            jitter: Jitter::default(),
            pcm: Box::new([0.0; 5760]),
            offset: 0,
            length: 0,
        });
        Ok(())
    }
    pub fn remove(&mut self, user: u64) {
        self.speakers.retain(|s| s.user != user);
    }
    pub fn user(&self, ssrc: u32) -> Option<u64> {
        self.speakers
            .iter()
            .find(|s| s.ssrc == ssrc)
            .map(|s| s.user)
    }
    pub fn push(&mut self, ssrc: u32, sequence: u16, opus: Vec<u8>) {
        if let Some(speaker) = self.speakers.iter_mut().find(|s| s.ssrc == ssrc) {
            speaker.jitter.push(sequence, opus);
        }
    }
    pub fn clear(&mut self) {
        for speaker in &mut self.speakers {
            speaker.jitter.clear();
            speaker.pcm.fill(0.0);
            speaker.offset = 0;
            speaker.length = 0;
        }
    }
    /// Mix one 20ms frame, preserving up to 120ms packets without bursting playback queues.
    pub fn pop(&mut self) -> (Option<Frame>, bool) {
        let mut output = [0.0; 960];
        let mut active = false;
        let mut heard = false;
        for speaker in &mut self.speakers {
            if speaker.offset == speaker.length {
                let Some(opus) = speaker.jitter.pop() else {
                    continue;
                };
                let limit = if opus.is_empty() {
                    960
                } else {
                    speaker.pcm.len()
                };
                let Ok(length) =
                    speaker
                        .decoder
                        .decode_float(&opus, &mut speaker.pcm[..limit], false)
                else {
                    continue;
                };
                speaker.offset = 0;
                speaker.length = length;
                heard |= !opus.is_empty() && opus != davey::OPUS_SILENCE_PACKET;
            }
            let end = (speaker.offset + 960).min(speaker.length);
            for (mixed, sample) in output.iter_mut().zip(&speaker.pcm[speaker.offset..end]) {
                if sample.is_finite() {
                    *mixed += sample;
                }
            }
            speaker.offset = end;
            active = true;
        }
        // ponytail: hard limiting bounds simultaneous speakers; add a soft limiter if clipping is audible.
        output
            .iter_mut()
            .for_each(|sample| *sample = sample.clamp(-1.0, 1.0));
        (active.then_some(output), heard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opus2::{Application, Encoder};
    fn opus(frequency: f32) -> Vec<u8> {
        let mut encoder = Encoder::new(48_000, Channels::Mono, Application::Voip).unwrap();
        let pcm: Frame = std::array::from_fn(|i| (i as f32 * frequency).sin() * 0.2);
        let mut encoded = [0; 1275];
        let length = encoder.encode_float(&pcm, &mut encoded).unwrap();
        encoded[..length].to_vec()
    }
    #[test]
    fn independent_streams_mix_on_one_clock_and_release_on_leave() {
        let mut together = Mixer::default();
        let mut alice = Mixer::default();
        let mut bob = Mixer::default();
        for (user, ssrc, single, frequency) in [(1, 11, &mut alice, 0.03), (2, 22, &mut bob, 0.08)]
        {
            together.announce(user, ssrc).unwrap();
            single.announce(user, ssrc).unwrap();
            // Same sequence from separate SSRCs must never collide or share decoder state.
            let packet = opus(frequency);
            together.push(ssrc, 7, packet.clone());
            single.push(ssrc, 7, packet);
        }
        assert!(together.announce(3, 11).is_err());
        for _ in 0..2 {
            assert!(together.pop().0.is_none());
            assert!(alice.pop().0.is_none());
            assert!(bob.pop().0.is_none());
        }
        let mixed = together.pop().0.unwrap();
        let a = alice.pop().0.unwrap();
        let b = bob.pop().0.unwrap();
        assert!(a.iter().any(|s| s.abs() > 0.01));
        assert!(b.iter().any(|s| s.abs() > 0.01));
        for i in 0..960 {
            assert!((mixed[i] - (a[i] + b[i]).clamp(-1.0, 1.0)).abs() < 0.0001);
        }
        together.remove(1);
        assert_eq!(together.user(11), None);
        together.clear();
        assert!(together.pop().0.is_none());
        for user in 3..65 {
            together.announce(user, user as u32 + 100).unwrap();
        }
        assert!(together.announce(65, 165).is_err());
        assert_eq!(together.speakers.len(), 63);
    }
    #[test]
    #[ignore = "synthetic release workload; run with --release --ignored --nocapture"]
    fn synthetic_mix_workload() {
        let packet = opus(0.05);
        for peers in [1, 8, 63] {
            let mut mixer = Mixer::default();
            for user in 1..=peers {
                mixer.announce(user, user as u32).unwrap();
            }
            let mut sequence = 0u16;
            let mut samples = Vec::new();
            for run in 0..6 {
                let start = std::time::Instant::now();
                for _ in 0..1000 {
                    for user in 1..=peers {
                        mixer.push(user as u32, sequence, packet.clone());
                    }
                    std::hint::black_box(mixer.pop());
                    sequence = sequence.wrapping_add(1);
                }
                if run > 0 {
                    samples.push(start.elapsed().as_micros());
                }
            }
            samples.sort_unstable();
            println!(
                "synthetic_mix peers={peers} ticks=1000 samples=5 median_us={} us_per_tick={:.2}",
                samples[2],
                samples[2] as f64 / 1000.0
            );
        }
    }
}
