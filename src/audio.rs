mod input_level;
pub use input_level::InputLevel;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use log::{debug, error, info};

use crate::AudioRecorder;

const OUTPUT_SAMPLE_RATE: u32 = 16_000;

pub struct CpalAudioRecorder {
    commands: mpsc::Sender<AudioCommand>,
}

struct ActiveRecording {
    _stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    source_sample_rate: u32,
    output_path: PathBuf,
}

enum AudioCommand {
    Start(mpsc::Sender<Result<(), String>>),
    Stop(mpsc::Sender<Result<PathBuf, String>>),
}

impl CpalAudioRecorder {
    pub fn new() -> Self {
        Self::with_input_level(InputLevel::default())
    }

    pub fn with_input_level(input_level: InputLevel) -> Self {
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("floating-whisprer-audio".to_owned())
            .spawn(move || audio_worker(receiver, input_level))
            .expect("unable to start audio worker");
        Self { commands }
    }
}

impl Default for CpalAudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioRecorder for CpalAudioRecorder {
    fn start(&mut self) -> Result<(), String> {
        request_audio_worker(&self.commands, AudioCommand::Start)
    }

    async fn stop(&mut self) -> Result<PathBuf, String> {
        let commands = self.commands.clone();
        tokio::task::spawn_blocking(move || request_audio_worker(&commands, AudioCommand::Stop))
            .await
            .map_err(|error| format!("Audio worker stopped unexpectedly: {error}"))?
    }
}

fn request_audio_worker<T>(
    commands: &mpsc::Sender<AudioCommand>,
    command: impl FnOnce(mpsc::Sender<Result<T, String>>) -> AudioCommand,
) -> Result<T, String> {
    let (response_sender, response_receiver) = mpsc::channel();
    commands
        .send(command(response_sender))
        .map_err(|_| "Audio worker is unavailable.".to_owned())?;
    response_receiver
        .recv()
        .map_err(|_| "Audio worker did not return a result.".to_owned())?
}

fn audio_worker(commands: mpsc::Receiver<AudioCommand>, input_level: InputLevel) {
    let mut active_recording = None;

    while let Ok(command) = commands.recv() {
        match command {
            AudioCommand::Start(response) => {
                let result = if active_recording.is_some() {
                    Err("Recording is already in progress.".to_owned())
                } else {
                    input_level.publish_rms(0.0);
                    create_input_stream(input_level.clone())
                        .map(|recording| active_recording = Some(recording))
                };
                let _ = response.send(result);
            }
            AudioCommand::Stop(response) => {
                let result = active_recording
                    .take()
                    .ok_or_else(|| "No recording is currently in progress.".to_owned())
                    .and_then(finish_recording);
                input_level.publish_rms(0.0);
                let _ = response.send(result);
            }
        }
    }
}

fn create_input_stream(input_level: InputLevel) -> Result<ActiveRecording, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| "No microphone input device is available.".to_owned())?;
    let supported_config = device
        .default_input_config()
        .map_err(|error| format!("Unable to read microphone configuration: {error}"))?;
    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.into();
    let samples = Arc::new(Mutex::new(Vec::new()));
    let callback_samples = samples.clone();
    let channels = usize::from(config.channels);
    let error_callback = |error| error!("audio input stream error: {error}");

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                append_f32_samples(data, channels, &callback_samples, &input_level)
            },
            error_callback,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                append_i16_samples(data, channels, &callback_samples, &input_level)
            },
            error_callback,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| {
                append_u16_samples(data, channels, &callback_samples, &input_level)
            },
            error_callback,
            None,
        ),
        unsupported => {
            return Err(format!(
                "Unsupported microphone sample format: {unsupported:?}"
            ));
        }
    }
    .map_err(|error| format!("Unable to create microphone stream: {error}"))?;

    stream
        .play()
        .map_err(|error| format!("Unable to start microphone recording: {error}"))?;
    info!(
        "audio recording started: source_rate={}, channels={}",
        config.sample_rate.0, config.channels
    );

    Ok(ActiveRecording {
        _stream: stream,
        samples,
        source_sample_rate: config.sample_rate.0,
        output_path: temporary_recording_path(),
    })
}

fn finish_recording(recording: ActiveRecording) -> Result<PathBuf, String> {
    let ActiveRecording {
        _stream: stream,
        samples,
        source_sample_rate,
        output_path,
    } = recording;
    drop(stream);

    let samples = samples
        .lock()
        .map_err(|_| "Unable to access recorded microphone samples.".to_owned())?;
    write_16khz_mono_wav(&output_path, &samples, source_sample_rate)?;
    debug!(
        "audio recording written: samples={}, path={}",
        samples.len(),
        output_path.display()
    );
    Ok(output_path)
}

fn append_f32_samples(
    samples: &[f32],
    channels: usize,
    destination: &Arc<Mutex<Vec<f32>>>,
    input_level: &InputLevel,
) {
    append_mono_frames(samples, channels, destination, input_level, |sample| {
        sample.clamp(-1.0, 1.0)
    });
}

fn append_i16_samples(
    samples: &[i16],
    channels: usize,
    destination: &Arc<Mutex<Vec<f32>>>,
    input_level: &InputLevel,
) {
    append_mono_frames(samples, channels, destination, input_level, |sample| {
        f32::from(sample) / f32::from(i16::MAX)
    });
}

fn append_u16_samples(
    samples: &[u16],
    channels: usize,
    destination: &Arc<Mutex<Vec<f32>>>,
    input_level: &InputLevel,
) {
    append_mono_frames(samples, channels, destination, input_level, |sample| {
        (f32::from(sample) / f32::from(u16::MAX)) * 2.0 - 1.0
    });
}

fn append_mono_frames<T: Copy>(
    samples: &[T],
    channels: usize,
    destination: &Arc<Mutex<Vec<f32>>>,
    input_level: &InputLevel,
    convert: impl Fn(T) -> f32,
) {
    if channels == 0 {
        return;
    }

    let Ok(mut destination) = destination.lock() else {
        return;
    };
    let mut energy = 0.0;
    let mut frame_count = 0;
    for frame in samples.chunks_exact(channels) {
        let mono = frame.iter().copied().map(&convert).sum::<f32>() / channels as f32;
        destination.push(mono);
        energy += mono * mono;
        frame_count += 1;
    }
    let rms = if frame_count == 0 {
        0.0
    } else {
        (energy / frame_count as f32).sqrt()
    };
    input_level.publish_rms(rms);
}

fn write_16khz_mono_wav(
    output_path: &std::path::Path,
    source_samples: &[f32],
    source_sample_rate: u32,
) -> Result<(), String> {
    let specification = hound::WavSpec {
        channels: 1,
        sample_rate: OUTPUT_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(output_path, specification)
        .map_err(|error| format!("Unable to create recording file: {error}"))?;

    for sample in resample(source_samples, source_sample_rate, OUTPUT_SAMPLE_RATE) {
        let sample = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        writer
            .write_sample(sample)
            .map_err(|error| format!("Unable to write recording file: {error}"))?;
    }

    writer
        .finalize()
        .map_err(|error| format!("Unable to finalize recording file: {error}"))
}

fn resample(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let output_length = samples.len() * target_rate as usize / source_rate as usize;
    (0..output_length)
        .map(|output_index| {
            let source_position = output_index as f64 * source_rate as f64 / target_rate as f64;
            let lower_index = source_position.floor() as usize;
            let upper_index = (lower_index + 1).min(samples.len() - 1);
            let fraction = (source_position - lower_index as f64) as f32;
            samples[lower_index] * (1.0 - fraction) + samples[upper_index] * fraction
        })
        .collect()
}

fn temporary_recording_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "floating-whisprer-{}-{timestamp}.wav",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metering_preserves_recorded_mono_samples() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let level = InputLevel::default();
        append_f32_samples(&[0.5, 0.5, -0.5, -0.5], 2, &samples, &level);
        assert_eq!(*samples.lock().unwrap(), vec![0.5, -0.5]);
        assert_eq!(level.rms(), 0.5);
        append_f32_samples(&[0.0, 0.0], 2, &samples, &level);
        assert_eq!(level.rms(), 0.0);
    }

    #[test]
    fn integer_microphone_formats_produce_normalized_levels() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let level = InputLevel::default();
        append_i16_samples(&[i16::MAX], 1, &samples, &level);
        assert_eq!(level.rms(), 1.0);
        append_u16_samples(&[0, u16::MAX], 1, &samples, &level);
        assert_eq!(level.rms(), 1.0);
        assert_eq!(*samples.lock().unwrap(), vec![1.0, -1.0, 1.0]);
    }

    #[test]
    fn writes_16khz_mono_16_bit_pcm_wav() {
        let output = temporary_recording_path();
        let source = vec![0.25; 48_000];

        write_16khz_mono_wav(&output, &source, 48_000).unwrap();

        let reader = hound::WavReader::open(&output).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.spec().sample_format, hound::SampleFormat::Int);
        assert_eq!(reader.duration(), 16_000);
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn preserves_samples_already_recorded_at_16khz() {
        let samples = vec![-1.0, -0.5, 0.0, 0.5, 1.0];
        assert_eq!(resample(&samples, 16_000, 16_000), samples);
    }
}
