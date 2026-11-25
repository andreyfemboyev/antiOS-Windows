use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::{
    fs::{self, File},
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub fn data_directory(override_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path.to_owned());
    }
    Ok(ProjectDirs::from("art", "antiOS", "antiOS")
        .context("no local application-data directory available; pass --data-dir")?
        .data_local_dir()
        .to_owned())
}

pub fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match File::open(path) {
        Ok(file) => serde_json::from_reader(BufReader::new(file))
            .with_context(|| format!("reading {}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error).with_context(|| format!("opening {}", path.display())),
    }
}

pub fn write_file(path: &Path, writer: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    writer(temporary.as_file_mut())?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .with_context(|| format!("committing {}", path.display()))?;
    Ok(())
}

pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    write_file(path, |file| Ok(serde_json::to_writer_pretty(file, value)?))
}

pub fn icon() -> Option<winit::window::Icon> {
    let mut pixels = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let p = glam::vec2(x as f32, y as f32);
            let center = glam::Vec2::splat(31.5);
            let mark = (p.distance(center) - 23.).abs() < 1.2
                || ((p.x + p.y - 63.).abs() < 1.6 && (12. ..52.).contains(&p.x))
                || ((p.y - 12.).abs() < 1.1 && (38. ..52.).contains(&p.x))
                || ((p.x - 51.).abs() < 1.1 && (12. ..27.).contains(&p.y));
            let signal = p.distance(glam::vec2(13.5, 14.5)) < 2.8;
            let color = if signal {
                [184, 255, 54, 255]
            } else if mark {
                [232, 230, 225, 255]
            } else {
                [8, 8, 8, if p.distance(center) < 29. { 255 } else { 0 }]
            };
            pixels.extend_from_slice(&color);
        }
    }
    winit::window::Icon::from_rgba(pixels, 64, 64).ok()
}
