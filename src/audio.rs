use crate::{
    entropy::{smooth, Phase},
    simulation::World,
};
use anyhow::{bail, Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SizedSample,
};
use std::{
    f32::consts::TAU,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

struct Parameters {
    gain: AtomicU32,
    entropy: AtomicU32,
    pressure: AtomicU32,
    silence: AtomicU32,
    resonance: AtomicU32,
    event: AtomicU32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            gain: 0.into(),
            entropy: 0.into(),
            pressure: 0.into(),
            silence: 1f32.to_bits().into(),
            resonance: 0.into(),
            event: 0.into(),
        }
    }
}

fn set(slot: &AtomicU32, value: f32) {
    slot.store(value.to_bits(), Ordering::Relaxed);
}
fn get(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

#[derive(Default)]
pub struct Audio {
    parameters: Arc<Parameters>,
    stream: Option<cpal::Stream>,
    audible: bool,
}

impl Audio {
    pub fn muted(&self) -> bool {
        !self.audible
    }

    pub fn toggle(&mut self) -> Result<()> {
        if self.stream.is_none() {
            self.stream = Some(open(Arc::clone(&self.parameters))?);
        }
        self.audible = !self.audible;
        if !self.audible {
            set(&self.parameters.gain, 0.);
        }
        Ok(())
    }

    pub fn follow(&self, world: &World, volume: f32, paused: bool) {
        if self.stream.is_none() {
            return;
        }
        let attributes = world.atmosphere;
        let silence = if attributes.phase == Phase::Genesis && attributes.cycle == 0 {
            1.
        } else {
            smooth(3., 38., world.cursor.idle) * 0.58 + attributes.fracture * 0.18
        };
        set(
            &self.parameters.gain,
            if self.audible && !paused {
                volume * 0.72
            } else {
                0.
            },
        );
        set(&self.parameters.entropy, attributes.entropy);
        set(&self.parameters.pressure, world.cursor.pressure);
        set(&self.parameters.silence, silence);
        set(&self.parameters.resonance, world.coupled);
        set(&self.parameters.event, world.sound_event);
    }
}

fn open(parameters: Arc<Parameters>) -> Result<cpal::Stream> {
    let device = cpal::default_host()
        .default_output_device()
        .context("no sound output available")?;
    let output = device
        .default_output_config()
        .context("opening default sound output")?;
    let config = output.config();
    use cpal::SampleFormat::*;
    let stream = match output.sample_format() {
        F32 => stream::<f32>(&device, &config, parameters),
        F64 => stream::<f64>(&device, &config, parameters),
        I8 => stream::<i8>(&device, &config, parameters),
        I16 => stream::<i16>(&device, &config, parameters),
        I32 => stream::<i32>(&device, &config, parameters),
        I64 => stream::<i64>(&device, &config, parameters),
        U8 => stream::<u8>(&device, &config, parameters),
        U16 => stream::<u16>(&device, &config, parameters),
        U32 => stream::<u32>(&device, &config, parameters),
        U64 => stream::<u64>(&device, &config, parameters),
        _ => bail!("unsupported sound output sample format"),
    }?;
    stream.play().context("starting sound")?;
    Ok(stream)
}

fn stream<T: Sample + SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    parameters: Arc<Parameters>,
) -> Result<cpal::Stream> {
    let sample_rate = config.sample_rate.0 as f32;
    let channels = usize::from(config.channels);
    let mut synthesizer = Synthesizer::new();
    Ok(device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            let target_gain = get(&parameters.gain);
            let entropy = get(&parameters.entropy);
            let pressure = get(&parameters.pressure);
            let silence = get(&parameters.silence);
            let resonance = get(&parameters.resonance);
            let event = get(&parameters.event);
            for frame in output.chunks_mut(channels) {
                let stereo = synthesizer.sample(
                    sample_rate,
                    target_gain,
                    [entropy, pressure, silence, resonance, event],
                );
                for (channel, sample) in frame.iter_mut().enumerate() {
                    *sample = T::from_sample(if channels == 1 {
                        (stereo[0] + stereo[1]) * 0.5
                    } else {
                        stereo[channel % 2]
                    });
                }
            }
        },
        |error| eprintln!("antiOS sound: {error}"),
        None,
    )?)
}

struct Synthesizer {
    phase: [f32; 4],
    drift: f32,
    dust: f32,
    random: u32,
    gain: f32,
    interval: f32,
    note: f32,
    envelope: f32,
    last_event: f32,
}

impl Synthesizer {
    fn new() -> Self {
        Self {
            phase: [0.; 4],
            drift: 0.,
            dust: 0.,
            random: 7_542_389,
            gain: 0.,
            interval: 2.7,
            note: 144.,
            envelope: 0.,
            last_event: 0.,
        }
    }

    fn sample(
        &mut self,
        rate: f32,
        volume: f32,
        [entropy, pressure, silence, resonance, event]: [f32; 5],
    ) -> [f32; 2] {
        let dt = 1. / rate;
        self.gain += (volume * (1. - silence) - self.gain) * dt * 12.;
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        let random = self.random as f32 * (1. / u32::MAX as f32);
        self.dust += (random * 2. - 1. - self.dust) * (dt * (128. + pressure * 650.)).min(1.);
        self.drift = (self.drift + dt * (0.09 + resonance * 0.046)).fract();
        let tide = (TAU * self.drift).sin();
        self.interval -= dt;
        if self.interval < 0. || (event > 0.68 && self.last_event <= 0.68) {
            self.envelope = (0.40 + pressure * 0.16 + event * 0.28) * (1. - silence);
            self.interval = 6. + random * 15. - entropy * 4.;
            self.note =
                [96., 127.2, 143.7, 174.6, 246.3][self.random as usize % 5] * (1. - event * 0.32);
        }
        self.last_event = event;
        self.envelope *= 1. - dt * (1.35 + entropy * 2.);
        for (phase, freq) in self.phase.iter_mut().zip([
            43.4 + tide * 0.6,
            65.1 + entropy * 4.,
            86.8 + tide * 1.25,
            self.note,
        ]) {
            *phase = (*phase + dt * freq).fract();
        }
        let [root, partial, shimmer, tone] = self.phase.map(|p| (p * TAU).sin());
        let drone = root * 0.115 + partial * (0.031 + resonance * 0.03);
        let grit = self.dust * (0.025 + entropy * 0.07 + event * 0.24) * (0.6 + tide * 0.4);
        let pulse = tone * self.envelope * 0.068;
        let room = shimmer * (0.022 + pressure * 0.033);
        [
            (drone + grit + pulse + room) * self.gain,
            (drone + grit * 0.84 + pulse * 0.86 - room) * self.gain,
        ]
    }
}
