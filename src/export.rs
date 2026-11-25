use crate::platform::write_file;
use anyhow::{ensure, Context, Result};
use std::{fs, path::Path, process::Command};

pub fn png<'a>(path: &Path, size: (u32, u32), rows: impl Iterator<Item = &'a [u8]>) -> Result<()> {
    write_file(path, |file| {
        let mut encoder = png::Encoder::new(file, size.0, size.1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        encoder.set_compression(png::Compression::Fast);
        let mut png = encoder.write_header()?;
        let mut stream = png.stream_writer()?;
        for row in rows {
            std::io::Write::write_all(&mut stream, row)?;
        }
        stream.finish()?;
        png.finish()?;
        Ok(())
    })
}

pub fn empty_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))?;
    ensure!(
        fs::read_dir(path)?.next().transpose()?.is_none(),
        "frame-sequence directory must be empty"
    );
    Ok(())
}

pub fn encode(frames: &Path, out: &Path, fps: u32) -> Result<()> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = tempfile::Builder::new()
        .suffix(".mp4")
        .tempfile_in(parent)?
        .into_temp_path();
    let result = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-framerate"])
        .arg(fps.to_string())
        .args(["-start_number", "0", "-i"])
        .arg(frames.join("%08d.png"))
        .args([
            "-vf",
            "pad=ceil(iw/2)*2:ceil(ih/2)*2",
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "16",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(&temporary)
        .status()
        .context("starting FFmpeg from PATH; PNG frames remain available")?;
    ensure!(
        result.success(),
        "FFmpeg exited with {result}; PNG frames remain available"
    );
    temporary.persist(out)?;
    Ok(())
}
