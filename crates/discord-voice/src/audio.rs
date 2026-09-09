//! Device I/O is created only after an explicit call reaches encrypted readiness.
//! CPAL callbacks use preallocated lock-free rings; codecs and channels stay off them.
use crate::Frame;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
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
#[derive(Default)]
pub struct Gate {
    pub ready: AtomicBool,
    pub muted: AtomicBool,
    pub deafened: AtomicBool,
    stopped: AtomicBool,
    failed: AtomicBool,
}
impl Gate {
    fn capture(&self) -> bool {
        self.ready.load(Ordering::Acquire)
            && !self.muted.load(Ordering::Acquire)
            && !self.stopped.load(Ordering::Acquire)
    }
    fn playback(&self) -> bool {
        self.ready.load(Ordering::Acquire)
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
            .name("dm-audio".into())
            .spawn(move || {
                let mut streams = None;
                let mut current = Devices::default();
                while !worker_gate.stopped.load(Ordering::Acquire) {
                    let next = selected.borrow_and_update().clone();
                    if current != next {
                        streams = None;
                        current = next;
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
                    if streams.is_none() {
                        match Streams::open(&current, worker_gate.clone()) {
                            Ok(value) => {
                                streams = Some(value);
                                emit(Ok(()));
                            }
                            Err(error) => {
                                emit(Err(error));
                                break;
                            }
                        }
                    }
                    if worker_gate.failed.load(Ordering::Acquire) {
                        emit(Err(
                            "Audio device stopped or disconnected; choose a device and call again",
                        ));
                        break;
                    }
                    let Some(active) = &mut streams else {
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
        self.gate.stopped.load(Ordering::Acquire) || self.gate.failed.load(Ordering::Acquire)
    }
    pub fn shutdown(mut self) -> mpsc::Receiver<()> {
        self.done.take().expect("audio owns completion")
    }
    pub fn set_devices(&self, settings: Devices) {
        self.settings.send_replace(settings);
        self.thread.unpark();
    }
    pub fn set_ready(&self, ready: bool) {
        self.gate.ready.store(ready, Ordering::Release);
        self.thread.unpark();
    }
    pub fn set_controls(&self, muted: bool, deafened: bool) {
        self.gate.muted.store(muted || deafened, Ordering::Release);
        self.gate.deafened.store(deafened, Ordering::Release);
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
    _input: cpal::Stream,
    _output: cpal::Stream,
    input: rtrb::Consumer<Frame>,
    output: rtrb::Producer<Frame>,
}
impl Streams {
    fn open(settings: &Devices, gate: Arc<Gate>) -> Result<Self, &'static str> {
        let host = cpal::default_host();
        let input = choose(&host, settings.input.as_deref(), true)?;
        let output = choose(&host, settings.output.as_deref(), false)?;
        let input_config = config(&input, true)?;
        let output_config = config(&output, false)?;
        let (input_write, input_read) = rtrb::RingBuffer::new(8);
        let (output_write, output_read) = rtrb::RingBuffer::new(8);
        let capture = Capture::new(input_config.sample_rate(), input_write);
        let render = Playback::new(output_config.sample_rate(), output_read);
        let input_stream = match input_config.sample_format() {
            cpal::SampleFormat::F32 => {
                input_stream::<f32>(&input, &input_config.config(), capture, gate.clone())
            }
            cpal::SampleFormat::I16 => {
                input_stream::<i16>(&input, &input_config.config(), capture, gate.clone())
            }
            cpal::SampleFormat::I32 => {
                input_stream::<i32>(&input, &input_config.config(), capture, gate.clone())
            }
            cpal::SampleFormat::U16 => {
                input_stream::<u16>(&input, &input_config.config(), capture, gate.clone())
            }
            _ => Err("Microphone sample format is not supported"),
        }?;
        let output_stream = match output_config.sample_format() {
            cpal::SampleFormat::F32 => {
                output_stream::<f32>(&output, &output_config.config(), render, gate.clone())
            }
            cpal::SampleFormat::I16 => {
                output_stream::<i16>(&output, &output_config.config(), render, gate.clone())
            }
            cpal::SampleFormat::I32 => {
                output_stream::<i32>(&output, &output_config.config(), render, gate.clone())
            }
            cpal::SampleFormat::U16 => {
                output_stream::<u16>(&output, &output_config.config(), render, gate.clone())
            }
            _ => Err("Speaker sample format is not supported"),
        }?;
        if !gate.ready.load(Ordering::Acquire) || gate.stopped.load(Ordering::Acquire) {
            return Err("Call ended before audio devices were ready");
        }
        output_stream
            .play()
            .map_err(|_| "Could not start speaker playback")?;
        input_stream
            .play()
            .map_err(|_| "Could not start microphone; check system microphone permission")?;
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
                if !gate.capture() {
                    capture.reset();
                    return;
                }
                for frame in data.chunks_exact(channels) {
                    let sample =
                        frame.iter().map(|v| v.to_sample::<f32>()).sum::<f32>() / channels as f32;
                    capture.sample(if sample.is_finite() {
                        sample.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    });
                }
            },
            move |_| {
                failure.failed.store(true, Ordering::Release);
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
                if !gate.playback() {
                    output.reset();
                    data.fill(T::from_sample(0.0));
                    return;
                }
                for frame in data.chunks_mut(channels) {
                    frame.fill(T::from_sample(output.sample()));
                }
            },
            move |_| {
                failure.failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| "Could not open speaker device")
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
    #[test]
    fn resampling_buffers_and_capture_gate_are_bounded_without_devices() {
        let gate = Gate::default();
        assert!(!gate.capture());
        gate.ready.store(true, Ordering::Release);
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
