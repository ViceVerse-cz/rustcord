//! Device I/O is created only after an explicit call reaches encrypted readiness.
//! CPAL callbacks use preallocated lock-free rings; codecs and channels stay off them.
use crate::Frame;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Devices {
    pub input: Option<String>,
    pub output: Option<String>,
}
pub struct DeviceList {
    pub inputs: Vec<(String, String)>,
    pub outputs: Vec<(String, String)>,
}
pub fn devices() -> Result<DeviceList, &'static str> {
    let host = cpal::default_host();
    let mut list = DeviceList {
        inputs: vec![],
        outputs: vec![],
    };
    for device in host
        .devices()
        .map_err(|_| "Audio devices are unavailable")?
        .take(64)
    {
        let Ok(id) = device.id() else {
            continue;
        };
        let id = id.to_string();
        if id.len() > 512 {
            continue;
        }
        let name: String = device.to_string().chars().take(128).collect();
        if device.supports_input() && list.inputs.len() < 32 {
            list.inputs.push((id.clone(), name.clone()));
        }
        if device.supports_output() && list.outputs.len() < 32 {
            list.outputs.push((id, name));
        }
    }
    Ok(list)
}
pub struct Gate {
    pub ready: AtomicBool,
    pub muted: AtomicBool,
    pub deafened: AtomicBool,
    stopped: AtomicBool,
    failed_revision: AtomicU64,
    revision: AtomicU64,
    acknowledged_revision: AtomicU64,
    input_enabled: AtomicBool,
    input_gain: AtomicU16,
    output_gain: AtomicU16,
}
impl Default for Gate {
    fn default() -> Self {
        Self {
            ready: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            deafened: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            failed_revision: AtomicU64::new(0),
            revision: AtomicU64::new(1),
            acknowledged_revision: AtomicU64::new(0),
            input_enabled: AtomicBool::new(true),
            input_gain: AtomicU16::new(100),
            output_gain: AtomicU16::new(100),
        }
    }
}
impl Gate {
    fn is_ready(&self) -> bool {
        let revision = self.revision.load(Ordering::Acquire);
        self.ready.load(Ordering::Acquire)
            && !self.stopped.load(Ordering::Acquire)
            && self.failed_revision.load(Ordering::Acquire) != revision
            && self.acknowledged_revision.load(Ordering::Acquire) == revision
    }
    fn acknowledge(&self, revision: u64) -> bool {
        if self.revision.load(Ordering::Acquire) != revision
            || !self.ready.load(Ordering::Acquire)
            || self.stopped.load(Ordering::Acquire)
        {
            return false;
        }
        self.acknowledged_revision
            .store(revision, Ordering::Release);
        self.is_ready()
    }
    fn capture(&self) -> bool {
        self.is_ready()
            && self.input_enabled.load(Ordering::Acquire)
            && !self.muted.load(Ordering::Acquire)
            && !self.stopped.load(Ordering::Acquire)
    }
    fn playback(&self) -> bool {
        self.is_ready()
            && !self.deafened.load(Ordering::Acquire)
            && !self.stopped.load(Ordering::Acquire)
    }
}
pub struct Audio {
    pub gate: Arc<Gate>,
    settings: tokio::sync::watch::Sender<Devices>,
    thread: std::thread::Thread,
    done: Option<mpsc::Receiver<()>>,
}
impl Audio {
    pub fn start(
        settings: Devices,
        capture: mpsc::SyncSender<Frame>,
        playback: mpsc::Receiver<Frame>,
        emit: impl Fn(Result<(), &'static str>) + Send + 'static,
    ) -> Result<Self, &'static str> {
        let gate = Arc::new(Gate::default());
        let worker_gate = gate.clone();
        let (settings, mut selected) = tokio::sync::watch::channel(settings);
        let (finished, done) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("voice-audio".into())
            .spawn(move || {
                let mut streams: Option<(u64, Streams)> = None;
                while !worker_gate.stopped.load(Ordering::Acquire) {
                    let revision = worker_gate.revision.load(Ordering::Acquire);
                    let current = selected.borrow_and_update().clone();
                    if streams
                        .as_ref()
                        .is_some_and(|(opened, _)| *opened != revision)
                    {
                        streams = None;
                    }
                    if !worker_gate.ready.load(Ordering::Acquire) {
                        streams = None;
                        for _ in 0..8 {
                            if playback.try_recv().is_err() {
                                break;
                            }
                        }
                        std::thread::park_timeout(Duration::from_millis(10));
                        continue;
                    }
                    if worker_gate.failed_revision.load(Ordering::Acquire) == revision
                        && worker_gate.revision.load(Ordering::Acquire) == revision
                    {
                        emit(Err(
                            "Audio device stopped or disconnected; choose a device and call again",
                        ));
                        break;
                    }
                    if streams.is_none() {
                        match Streams::open(&current, worker_gate.clone(), revision) {
                            Ok(value) => {
                                if !worker_gate.acknowledge(revision) {
                                    continue;
                                }
                                streams = Some((revision, value));
                                emit(Ok(()));
                            }
                            Err(error) => {
                                if worker_gate.revision.load(Ordering::Acquire) != revision
                                    || !worker_gate.ready.load(Ordering::Acquire)
                                {
                                    continue;
                                }
                                emit(Err(error));
                                break;
                            }
                        }
                    }
                    let Some((_, active)) = &mut streams else {
                        continue;
                    };
                    for _ in 0..8 {
                        let Ok(frame) = active.input.pop() else {
                            break;
                        };
                        if worker_gate.capture() {
                            let _ = capture.try_send(frame);
                        }
                    }
                    for _ in 0..8 {
                        let Ok(frame) = playback.try_recv() else {
                            break;
                        };
                        if worker_gate.playback() {
                            let _ = active.output.push(frame);
                        }
                    }
                    std::thread::park_timeout(Duration::from_millis(5));
                }
                worker_gate.stopped.store(true, Ordering::Release);
                drop(streams);
                let _ = finished.send(());
            })
            .map_err(|_| "Could not start audio device worker")?;
        Ok(Self {
            gate,
            settings,
            thread: thread.thread().clone(),
            done: Some(done),
        })
    }
    pub fn is_stopped(&self) -> bool {
        self.gate.stopped.load(Ordering::Acquire)
            || self.gate.failed_revision.load(Ordering::Acquire)
                == self.gate.revision.load(Ordering::Acquire)
    }
    pub fn shutdown(mut self) -> mpsc::Receiver<()> {
        self.done.take().expect("audio owns completion")
    }
    pub fn set_devices(&self, settings: Devices) {
        self.settings.send_if_modified(|current| {
            if *current == settings {
                return false;
            }
            self.gate.revision.fetch_add(1, Ordering::AcqRel);
            *current = settings;
            true
        });
        self.thread.unpark();
    }
    pub fn set_ready(&self, ready: bool) {
        if self.gate.ready.swap(ready, Ordering::AcqRel) && !ready {
            self.gate.revision.fetch_add(1, Ordering::AcqRel);
        }
        self.thread.unpark();
    }
    /// Permission-driven microphone availability, independent of mute and push-to-talk.
    pub fn set_input_enabled(&self, enabled: bool) {
        if self.gate.input_enabled.swap(enabled, Ordering::AcqRel) != enabled {
            self.gate.revision.fetch_add(1, Ordering::AcqRel);
            self.thread.unpark();
        }
    }
    /// True only after streams for the current device/security revision have opened.
    pub fn is_ready(&self) -> bool {
        self.gate.is_ready()
    }
    pub fn set_controls(&self, muted: bool, deafened: bool) {
        self.gate.muted.store(muted || deafened, Ordering::Release);
        self.gate.deafened.store(deafened, Ordering::Release);
    }
    /// Adjusts software gain without reopening devices. Defaults to 100%; clamps to 0..=200%.
    pub fn set_gain(&self, input_percent: u16, output_percent: u16) {
        self.gate
            .input_gain
            .store(input_percent.min(200), Ordering::Relaxed);
        self.gate
            .output_gain
            .store(output_percent.min(200), Ordering::Relaxed);
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.gate.stopped.store(true, Ordering::Release);
        self.gate.ready.store(false, Ordering::Release);
        self.thread.unpark();
    }
}

struct Streams {
    _input: Option<cpal::Stream>,
    _output: cpal::Stream,
    input: rtrb::Consumer<Frame>,
    output: rtrb::Producer<Frame>,
}
impl Streams {
    fn open(settings: &Devices, gate: Arc<Gate>, revision: u64) -> Result<Self, &'static str> {
        let host = cpal::default_host();
        let output = choose(&host, settings.output.as_deref(), false)?;
        let output_config = config(&output, false)?;
        let (input_write, input_read) = rtrb::RingBuffer::new(8);
        let (output_write, output_read) = rtrb::RingBuffer::new(8);
        let render = Playback::new(output_config.sample_rate(), output_read);
        let input_stream = if gate.input_enabled.load(Ordering::Acquire) {
            let input = choose(&host, settings.input.as_deref(), true)?;
            let input_config = config(&input, true)?;
            let capture = Capture::new(input_config.sample_rate(), input_write);
            Some(match input_config.sample_format() {
                cpal::SampleFormat::F32 => input_stream::<f32>(
                    &input,
                    &input_config.config(),
                    capture,
                    gate.clone(),
                    revision,
                ),
                cpal::SampleFormat::I16 => input_stream::<i16>(
                    &input,
                    &input_config.config(),
                    capture,
                    gate.clone(),
                    revision,
                ),
                cpal::SampleFormat::I32 => input_stream::<i32>(
                    &input,
                    &input_config.config(),
                    capture,
                    gate.clone(),
                    revision,
                ),
                cpal::SampleFormat::U16 => input_stream::<u16>(
                    &input,
                    &input_config.config(),
                    capture,
                    gate.clone(),
                    revision,
                ),
                _ => Err("Microphone sample format is not supported"),
            }?)
        } else {
            None
        };
        let output_stream = match output_config.sample_format() {
            cpal::SampleFormat::F32 => output_stream::<f32>(
                &output,
                &output_config.config(),
                render,
                gate.clone(),
                revision,
            ),
            cpal::SampleFormat::I16 => output_stream::<i16>(
                &output,
                &output_config.config(),
                render,
                gate.clone(),
                revision,
            ),
            cpal::SampleFormat::I32 => output_stream::<i32>(
                &output,
                &output_config.config(),
                render,
                gate.clone(),
                revision,
            ),
            cpal::SampleFormat::U16 => output_stream::<u16>(
                &output,
                &output_config.config(),
                render,
                gate.clone(),
                revision,
            ),
            _ => Err("Speaker sample format is not supported"),
        }?;
        if !gate.ready.load(Ordering::Acquire)
            || gate.stopped.load(Ordering::Acquire)
            || gate.revision.load(Ordering::Acquire) != revision
        {
            return Err("Call ended before audio devices were ready");
        }
        output_stream
            .play()
            .map_err(|_| "Could not start speaker playback")?;
        if let Some(input_stream) = &input_stream {
            input_stream
                .play()
                .map_err(|_| "Could not start microphone; check system microphone permission")?;
        }
        Ok(Self {
            _input: input_stream,
            _output: output_stream,
            input: input_read,
            output: output_write,
        })
    }
}
fn choose(host: &cpal::Host, id: Option<&str>, input: bool) -> Result<cpal::Device, &'static str> {
    if let Some(id) = id {
        let id = id.parse().map_err(|_| "Invalid audio device selection")?;
        host.device_by_id(&id)
            .ok_or("Selected audio device is no longer available")
    } else {
        if input {
            host.default_input_device()
        } else {
            host.default_output_device()
        }
        .ok_or("No default audio device is available")
    }
}
fn config(device: &cpal::Device, input: bool) -> Result<cpal::SupportedStreamConfig, &'static str> {
    let supported: Vec<_> = if input {
        device
            .supported_input_configs()
            .map_err(|_| "Microphone formats are unavailable")?
            .take(64)
            .collect()
    } else {
        device
            .supported_output_configs()
            .map_err(|_| "Speaker formats are unavailable")?
            .take(64)
            .collect()
    };
    if let Some(config) = supported
        .into_iter()
        .filter(|c| c.channels() > 0 && c.channels() <= 8)
        .filter(|c| supported_format(c.sample_format()))
        .filter_map(|c| c.try_with_sample_rate(48_000))
        .min_by_key(|c| c.channels())
    {
        return Ok(config);
    }
    let config = if input {
        device.default_input_config()
    } else {
        device.default_output_config()
    }
    .map_err(|_| "Default audio format is unavailable")?;
    if config.channels() == 0
        || config.channels() > 8
        || !(8_000..=192_000).contains(&config.sample_rate())
        || !supported_format(config.sample_format())
    {
        return Err("Audio device format is unsupported; choose another device");
    }
    Ok(config)
}
fn supported_format(format: cpal::SampleFormat) -> bool {
    matches!(
        format,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::I32
            | cpal::SampleFormat::U16
    )
}
fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut capture: Capture,
    gate: Arc<Gate>,
    revision: u64,
) -> Result<cpal::Stream, &'static str>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = usize::from(config.channels);
    let failure = gate.clone();
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                capture.process(data, channels, &gate);
            },
            move |_| {
                failure
                    .failed_revision
                    .fetch_max(revision, Ordering::AcqRel);
            },
            None,
        )
        .map_err(|_| "Could not open microphone; check device and microphone permission")
}
fn output_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut output: Playback,
    gate: Arc<Gate>,
    revision: u64,
) -> Result<cpal::Stream, &'static str>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let failure = gate.clone();
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| {
                output.render(data, channels, &gate);
            },
            move |_| {
                failure
                    .failed_revision
                    .fetch_max(revision, Ordering::AcqRel);
            },
            None,
        )
        .map_err(|_| "Could not open speaker device")
}
fn amplify(sample: f32, gain: f32) -> f32 {
    if sample.is_finite() {
        (sample * gain).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}
// ponytail: linear conversion is a fallback for devices lacking 48 kHz; use a band-limited resampler if aliasing is measured to matter.
struct Capture {
    previous: Option<f32>,
    phase: f64,
    step: f64,
    frame: Frame,
    index: usize,
    output: rtrb::Producer<Frame>,
}
impl Capture {
    fn process<T: cpal::SizedSample>(&mut self, data: &[T], channels: usize, gate: &Gate)
    where
        f32: cpal::FromSample<T>,
    {
        if !gate.capture() {
            self.reset();
            return;
        }
        let gain = f32::from(gate.input_gain.load(Ordering::Relaxed)) / 100.0;
        for frame in data.chunks_exact(channels) {
            let sample = frame.iter().map(|v| v.to_sample::<f32>()).sum::<f32>() / channels as f32;
            self.sample(amplify(sample, gain));
        }
    }
    fn new(rate: u32, output: rtrb::Producer<Frame>) -> Self {
        Self {
            previous: None,
            phase: 0.0,
            step: f64::from(rate) / 48_000.0,
            frame: [0.0; 960],
            index: 0,
            output,
        }
    }
    fn reset(&mut self) {
        self.previous = None;
        self.phase = 0.0;
        self.index = 0;
        self.frame.fill(0.0);
    }
    fn sample(&mut self, sample: f32) {
        if let Some(previous) = self.previous {
            while self.phase < 1.0 {
                self.frame[self.index] = previous + (sample - previous) * self.phase as f32;
                self.index += 1;
                if self.index == 960 {
                    let _ = self.output.push(self.frame);
                    self.index = 0;
                }
                self.phase += self.step;
            }
            self.phase -= 1.0;
        }
        self.previous = Some(sample);
    }
}
struct Playback {
    input: rtrb::Consumer<Frame>,
    frame: Frame,
    index: usize,
    previous: f32,
    next: f32,
    phase: f64,
    step: f64,
}
impl Playback {
    fn render<T: cpal::SizedSample + cpal::FromSample<f32>>(
        &mut self,
        data: &mut [T],
        channels: usize,
        gate: &Gate,
    ) {
        if !gate.playback() {
            self.reset();
            data.fill(T::from_sample(0.0));
            return;
        }
        let gain = f32::from(gate.output_gain.load(Ordering::Relaxed)) / 100.0;
        for frame in data.chunks_mut(channels) {
            frame.fill(T::from_sample(amplify(self.sample(), gain)));
        }
    }
    fn new(rate: u32, input: rtrb::Consumer<Frame>) -> Self {
        Self {
            input,
            frame: [0.0; 960],
            index: 960,
            previous: 0.0,
            next: 0.0,
            phase: 1.0,
            step: 48_000.0 / f64::from(rate),
        }
    }
    fn reset(&mut self) {
        for _ in 0..8 {
            if self.input.pop().is_err() {
                break;
            }
        }
        self.frame.fill(0.0);
        self.index = 960;
        self.previous = 0.0;
        self.next = 0.0;
        self.phase = 1.0;
    }
    fn pull(&mut self) -> f32 {
        if self.index == 960 {
            self.frame = self.input.pop().unwrap_or([0.0; 960]);
            self.index = 0;
        }
        let value = self.frame[self.index];
        self.index += 1;
        if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }
    fn sample(&mut self) -> f32 {
        while self.phase >= 1.0 {
            self.previous = self.next;
            self.next = self.pull();
            self.phase -= 1.0;
        }
        let value = self.previous + (self.next - self.previous) * self.phase as f32;
        self.phase += self.step;
        value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn audio_without_devices() -> Audio {
        let (settings, _) = tokio::sync::watch::channel(Devices::default());
        Audio {
            gate: Arc::new(Gate::default()),
            settings,
            thread: std::thread::current(),
            done: None,
        }
    }

    #[test]
    fn device_readiness_rejects_stale_open_and_fast_security_transitions() {
        let audio = audio_without_devices();
        audio.set_ready(true);
        let first = audio.gate.revision.load(Ordering::Acquire);
        assert!(!audio.is_ready());
        assert!(audio.gate.acknowledge(first));
        assert!(audio.is_ready() && audio.gate.capture() && audio.gate.playback());
        audio.set_ready(false);
        audio.set_ready(true); // Worker has not observed the intervening pause.
        let second = audio.gate.revision.load(Ordering::Acquire);
        assert_ne!(first, second);
        assert!(!audio.is_ready() && !audio.gate.capture() && !audio.gate.playback());
        assert!(!audio.gate.acknowledge(first));
        assert!(audio.gate.acknowledge(second));
        audio.set_ready(true);
        assert_eq!(audio.gate.revision.load(Ordering::Acquire), second);
        audio.set_devices(Devices::default());
        assert_eq!(audio.gate.revision.load(Ordering::Acquire), second);
        audio.set_devices(Devices {
            input: Some("synthetic-device".into()),
            output: None,
        });
        let third = audio.gate.revision.load(Ordering::Acquire);
        assert_ne!(second, third);
        assert!(!audio.is_ready() && !audio.gate.capture());
        assert!(!audio.gate.acknowledge(second));
        audio.gate.failed_revision.store(second, Ordering::Release);
        assert!(!audio.is_stopped()); // A retired device cannot fail the new configuration.
        assert!(audio.gate.acknowledge(third));
        assert!(audio.is_ready());
        audio.gate.failed_revision.store(third, Ordering::Release);
        assert!(audio.is_stopped());
        assert!(!audio.is_ready());
    }

    #[test]
    fn listen_only_keeps_playback_without_input_and_mute_does_not_reopen_devices() {
        let audio = audio_without_devices();
        audio.set_input_enabled(false);
        audio.set_ready(true);
        let revision = audio.gate.revision.load(Ordering::Acquire);
        assert!(audio.gate.acknowledge(revision));
        assert!(audio.is_ready() && audio.gate.playback());
        assert!(!audio.gate.capture());
        let (send, mut received) = rtrb::RingBuffer::new(8);
        let mut capture = Capture::new(48_000, send);
        capture.process(&[0.25_f32; 961], 1, &audio.gate);
        assert!(received.pop().is_err());
        audio.set_controls(true, false);
        audio.set_controls(false, false);
        audio.set_input_enabled(false);
        assert_eq!(audio.gate.revision.load(Ordering::Acquire), revision);
        assert!(!audio.gate.capture() && audio.gate.playback());
        audio.set_input_enabled(true);
        let next = audio.gate.revision.load(Ordering::Acquire);
        assert_ne!(next, revision);
        assert!(!audio.is_ready() && !audio.gate.capture());
        assert!(audio.gate.acknowledge(next));
        assert!(audio.gate.capture() && audio.gate.playback());
    }

    #[test]
    fn callback_gain_defaults_clamps_and_sanitizes_without_devices() {
        let audio = audio_without_devices();
        assert_eq!(audio.gate.input_gain.load(Ordering::Relaxed), 100);
        assert_eq!(audio.gate.output_gain.load(Ordering::Relaxed), 100);
        audio.set_ready(true);
        assert!(
            audio
                .gate
                .acknowledge(audio.gate.revision.load(Ordering::Acquire))
        );
        for (percent, expected) in [(0, 0.0), (100, 0.75), (200, 1.0), (u16::MAX, 1.0)] {
            audio.set_gain(percent, percent);
            for sample in [0.75_f32, -0.75] {
                let expected = expected * sample.signum();
                let (send, mut receive) = rtrb::RingBuffer::new(8);
                let mut capture = Capture::new(48_000, send);
                capture.process(&[sample; 961], 1, &audio.gate);
                assert!(receive.pop().unwrap().iter().all(|s| *s == expected));
                let (mut send, receive) = rtrb::RingBuffer::new(8);
                send.push([sample; 960]).unwrap();
                let mut playback = Playback::new(48_000, receive);
                let mut rendered = [0.0_f32; 1920];
                playback.render(&mut rendered, 2, &audio.gate);
                assert_eq!(&rendered[..2], &[0.0, 0.0]);
                assert!(rendered[2..].iter().all(|s| *s == expected));
            }
        }
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let (send, mut receive) = rtrb::RingBuffer::new(8);
            let mut capture = Capture::new(48_000, send);
            capture.process(&[invalid; 961], 1, &audio.gate);
            assert_eq!(receive.pop().unwrap(), [0.0; 960]);
            let (mut send, receive) = rtrb::RingBuffer::new(8);
            send.push([invalid; 960]).unwrap();
            let mut playback = Playback::new(48_000, receive);
            let mut rendered = [1.0_f32; 960];
            playback.render(&mut rendered, 1, &audio.gate);
            assert_eq!(rendered, [0.0; 960]);
        }
    }

    #[test]
    fn runtime_gain_updates_preserve_gates_and_discard_buffered_audio() {
        let audio = audio_without_devices();
        let (capture_send, mut captured) = rtrb::RingBuffer::new(8);
        let mut capture = Capture::new(48_000, capture_send);
        let (mut playback_send, playback_receive) = rtrb::RingBuffer::new(8);
        let mut playback = Playback::new(48_000, playback_receive);
        let mut rendered = [1.0_f32; 2];
        playback_send.push([0.25; 960]).unwrap();
        capture.process(&[0.25; 961], 1, &audio.gate);
        playback.render(&mut rendered, 1, &audio.gate);
        assert!(captured.pop().is_err());
        assert_eq!(rendered, [0.0; 2]);
        audio.set_ready(true);
        assert!(
            audio
                .gate
                .acknowledge(audio.gate.revision.load(Ordering::Acquire))
        );
        playback.render(&mut rendered, 1, &audio.gate);
        assert_eq!(rendered, [0.0; 2]);

        // The same callback state observes new controls; no stream needs recreation.
        playback.reset();
        playback_send.push([0.25; 960]).unwrap();
        playback.render(&mut rendered, 1, &audio.gate);
        assert_eq!(rendered, [0.0, 0.25]);
        audio.set_gain(200, 0);
        capture.process(&[0.25; 961], 1, &audio.gate);
        assert_eq!(captured.pop().unwrap(), [0.5; 960]);
        playback.render(&mut rendered, 1, &audio.gate);
        assert_eq!(rendered, [0.0; 2]);
        audio.set_gain(0, 200);
        capture.process(&[0.25; 960], 1, &audio.gate);
        let frame = captured.pop().unwrap();
        assert_eq!(frame[0], 0.5); // One already captured resampler endpoint.
        assert!(frame[1..].iter().all(|s| *s == 0.0));
        playback.render(&mut rendered, 1, &audio.gate);
        assert_eq!(rendered, [0.5; 2]);

        audio.set_controls(true, false);
        capture.process(&[0.75; 961], 1, &audio.gate);
        assert!(captured.pop().is_err());
        playback.render(&mut rendered, 1, &audio.gate);
        assert_eq!(rendered, [0.5; 2]);
        audio.set_controls(false, true);
        playback_send.push([0.75; 960]).unwrap();
        capture.process(&[0.75; 961], 1, &audio.gate);
        playback.render(&mut rendered, 1, &audio.gate);
        assert!(captured.pop().is_err());
        assert_eq!(rendered, [0.0; 2]);
        audio.set_controls(false, false);
        audio.set_gain(100, 100);
        capture.process(&[0.25; 961], 1, &audio.gate);
        assert_eq!(captured.pop().unwrap(), [0.25; 960]);
        playback.render(&mut rendered, 1, &audio.gate);
        assert_eq!(rendered, [0.0; 2]);
        audio.gate.stopped.store(true, Ordering::Release);
        capture.process(&[0.75; 961], 1, &audio.gate);
        playback_send.push([0.75; 960]).unwrap();
        playback.render(&mut rendered, 1, &audio.gate);
        assert!(captured.pop().is_err());
        assert_eq!(rendered, [0.0; 2]);
    }

    #[test]
    fn resampling_buffers_and_capture_gate_are_bounded_without_devices() {
        let gate = Gate::default();
        assert!(!gate.capture());
        gate.ready.store(true, Ordering::Release);
        assert!(gate.acknowledge(gate.revision.load(Ordering::Acquire)));
        assert!(gate.capture());
        gate.muted.store(true, Ordering::Release);
        assert!(!gate.capture());
        assert!(gate.playback());
        gate.deafened.store(true, Ordering::Release);
        assert!(!gate.playback());
        for rate in [44_100, 48_000, 96_000] {
            let (send, mut receive) = rtrb::RingBuffer::new(8);
            let mut capture = Capture::new(rate, send);
            let mut count = 0;
            for _ in 0..rate {
                capture.sample(0.25);
                while let Ok(frame) = receive.pop() {
                    assert!(frame.iter().all(|s| (*s - 0.25).abs() < 0.0001));
                    count += 960;
                }
            }
            assert!((47_040..=48_000).contains(&count));
        }
        let (mut send, receive) = rtrb::RingBuffer::new(8);
        for _ in 0..8 {
            send.push([0.5; 960]).unwrap();
        }
        assert!(send.push([0.5; 960]).is_err());
        let mut playback = Playback::new(44_100, receive);
        for _ in 0..7000 {
            assert!(playback.sample().is_finite());
        }
        playback.reset();
        assert_eq!(playback.sample(), 0.0);
    }
}
