use anyhow::{ensure, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Auto,
    Low,
    Medium,
    High,
}

impl Quality {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Auto,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }

    pub const fn scale(self) -> f32 {
        match self {
            Self::Low => 0.67,
            Self::Medium => 0.85,
            _ => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Backend {
    #[default]
    Auto,
    Vulkan,
    Dx12,
    Gl,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Preferences {
    pub quality: Quality,
    pub reduced_motion: bool,
    pub volume: f32,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            quality: Quality::Auto,
            reduced_motion: false,
            volume: 0.48,
        }
    }
}

impl Preferences {
    pub fn normalize(&mut self) {
        self.volume = if self.volume.is_finite() {
            self.volume.clamp(0., 1.)
        } else {
            Self::default().volume
        };
    }
}

#[derive(Parser, Debug)]
#[command(version, about = "antiOS — An operating system without a purpose.")]
pub struct Args {
    /// World seed (decimal).
    #[arg(long)]
    pub seed: Option<u64>,
    /// Start a fullscreen, autonomous installation.
    #[arg(long)]
    pub exhibition: bool,
    /// End the installation after this many seconds. Escape always exits fullscreen.
    #[arg(long, requires = "exhibition", value_parser = clap::value_parser!(u32).range(1..))]
    pub duration: Option<u32>,
    /// Keep the cursor visible in exhibition mode.
    #[arg(long, requires = "exhibition")]
    pub show_cursor: bool,
    /// Render a single PNG without opening a desktop window.
    #[arg(long, conflicts_with_all = ["frames", "exhibition"])]
    pub render: Option<PathBuf>,
    /// Export numbered PNGs to a new or empty directory.
    #[arg(long, conflicts_with = "exhibition")]
    pub frames: Option<PathBuf>,
    /// Advance this many output frames before rendering.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..=360000))]
    pub at_frame: u32,
    /// Number of images in a frame-sequence export.
    #[arg(long, requires = "frames", value_parser = clap::value_parser!(u32).range(1..=360000))]
    pub count: Option<u32>,
    /// Output frames per second.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(1..=120))]
    pub fps: u32,
    /// Output or initial window size (WIDTHxHEIGHT).
    #[arg(long, value_parser = parse_size)]
    pub size: Option<(u32, u32)>,
    /// Omit exhibition labels and controls from PNGs.
    #[arg(long)]
    pub clean: bool,
    /// After exporting a sequence, invoke FFmpeg to encode an MP4.
    #[arg(long, requires = "frames")]
    pub mp4: Option<PathBuf>,
    /// Rendering quality. Auto adapts to the graphics adapter and frame time.
    #[arg(long)]
    pub quality: Option<Quality>,
    /// Graphics backend. Auto tries the primary native backend, then OpenGL.
    #[arg(long, default_value = "auto")]
    pub backend: Backend,
    /// Access preferences, memory and captures in this directory.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Render a motion-reduced world.
    #[arg(long)]
    pub reduced_motion: bool,
}

impl Args {
    pub fn validate(&self) -> Result<()> {
        let export = self.render.is_some() || self.frames.is_some();
        ensure!(
            export || self.at_frame == 0,
            "--at-frame requires --render or --frames"
        );
        Ok(())
    }
}

fn parse_size(value: &str) -> Result<(u32, u32), String> {
    let error = || "expected WIDTHxHEIGHT, each between 320 and 8192".to_owned();
    let (width, height) = value.split_once(['x', 'X']).ok_or_else(error)?;
    let w: u32 = width.parse().map_err(|_| error())?;
    let h: u32 = height.parse().map_err(|_| error())?;
    if !(320..=8192).contains(&w) || !(320..=8192).contains(&h) {
        return Err(error());
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_and_config_defaults() {
        assert_eq!(parse_size("3840x2400"), Ok((3840, 2400)));
        assert!(parse_size("0x900").is_err());
        assert!(parse_size("1280").is_err());
        assert!(Args::try_parse_from(["antios", "--duration", "30"]).is_err());
        let mut old: Preferences = serde_json::from_str(r#"{"volume":4}"#).unwrap();
        old.normalize();
        assert_eq!(old.volume, 1.);
        assert_eq!(old.quality, Quality::Auto);
    }
}
