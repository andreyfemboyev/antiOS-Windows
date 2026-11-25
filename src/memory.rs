use crate::{
    entropy::smooth,
    interaction::Trace,
    organisms::Organism,
    platform::{read_json, write_json},
    simulation::{Generator, Impression, World},
};
use anyhow::{ensure, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
struct Session {
    id: u64,
    seed: u64,
    seconds: f32,
    entropy: f32,
    survivors: [Organism; 7],
    impressions: Vec<Impression>,
    traces: Vec<Trace>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Memory {
    version: u32,
    pub seconds: f64,
    entropy: f32,
    sessions: Vec<Session>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            version: 1,
            seconds: 0.,
            entropy: 0.,
            sessions: Vec::new(),
        }
    }
}

impl Memory {
    pub fn load(directory: &Path) -> Result<Self> {
        let mut data: Self = read_json(&directory.join("memory.json"))?;
        ensure!(
            data.version == 1,
            "unsupported antiOS memory version {}",
            data.version
        );
        ensure!(
            data.seconds.is_finite() && data.entropy.is_finite(),
            "invalid memory totals"
        );
        data.sessions.truncate(12);
        Ok(data)
    }

    pub fn recall(&self, world: &mut World) {
        world.accumulated = self.entropy.max(0.);
        if let Some(previous) = self.sessions.first() {
            for (body, survivor) in world.organisms.iter_mut().zip(&previous.survivors) {
                let center = survivor
                    .position
                    .clamp(glam::Vec2::splat(0.08), glam::Vec2::splat(0.93));
                body.position = body.position.lerp(center, 0.34);
                body.memory[5..].copy_from_slice(&survivor.memory[..7]);
            }
        }
        for (age, previous) in self.sessions.iter().take(5).enumerate() {
            let forgetting = 0.012 + 0.15 * smooth(0., 8., age as f32);
            let mut rng = Generator::new(previous.seed ^ world.seed);
            for historical in previous.impressions.iter().rev().take(3) {
                let mut memory = historical.clone();
                for center in &mut memory.centers {
                    *center = (*center + rng.axis() * forgetting)
                        .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
                }
                world.inherited.push(memory);
            }
            if !previous.traces.is_empty() {
                let mut echo = previous
                    .impressions
                    .last()
                    .cloned()
                    .unwrap_or_else(|| world.imprint());
                for (center, trace) in echo.centers.iter_mut().zip(previous.traces.iter().rev()) {
                    *center = (trace.position + rng.axis() * forgetting)
                        .clamp(glam::Vec2::ZERO, glam::Vec2::ONE);
                }
                world.inherited.push(echo);
            }
        }
    }
}

pub struct Journal {
    directory: PathBuf,
    id: u64,
    last_frame: u64,
    last_entropy: f32,
}

impl Journal {
    pub fn new(directory: PathBuf, id: u64, world: &World) -> Self {
        Self {
            directory,
            id,
            last_frame: 0,
            last_entropy: world.accumulated,
        }
    }

    // Used by periodic checkpoints and every sanctioned exit path.
    pub fn checkpoint(&mut self, world: &World) -> Result<()> {
        if self.last_frame == world.frame {
            return Ok(());
        }
        fs::create_dir_all(&self.directory)?;
        let guard = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(self.directory.join("memory.lock"))?;
        FileExt::lock_exclusive(&guard)?;
        let mut memory = Memory::load(&self.directory)?;
        let mut impressions = world.impressions.clone();
        impressions.push(world.imprint());
        let previous = memory.sessions.iter().find(|s| s.id == self.id);
        let entropy =
            previous.map_or(0., |s| s.entropy) + (world.accumulated - self.last_entropy).max(0.);
        memory.seconds += (world.frame - self.last_frame) as f64 / 60.;
        memory.entropy += (world.accumulated - self.last_entropy).max(0.);
        memory.sessions.retain(|s| s.id != self.id);
        memory.sessions.insert(
            0,
            Session {
                id: self.id,
                seed: world.seed,
                seconds: world.time(),
                entropy,
                survivors: world.organisms.clone(),
                impressions,
                traces: world.traces.clone(),
            },
        );
        memory.sessions.truncate(12);
        write_json(&self.directory.join("memory.json"), &memory)?;
        self.last_entropy = world.accumulated;
        self.last_frame = world.frame;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_exit_checkpoints_and_overlapping_sessions_preserve_time() {
        let temporary = tempfile::tempdir().unwrap();
        let mut first = World::new(100);
        let mut second = World::new(200);
        let mut a = Journal::new(temporary.path().into(), 1, &first);
        let mut b = Journal::new(temporary.path().into(), 2, &second);
        for _ in 0..180 {
            first.step();
        }
        for _ in 0..60 {
            second.step();
        }
        a.checkpoint(&first).unwrap();
        b.checkpoint(&second).unwrap();
        a.checkpoint(&first).unwrap();
        for _ in 0..60 {
            first.step();
        }
        a.checkpoint(&first).unwrap();
        let memory = Memory::load(temporary.path()).unwrap();
        assert_eq!(memory.seconds, 5.);
        assert_eq!(memory.sessions.len(), 2);
        let mut resumed = World::new(34);
        memory.recall(&mut resumed);
        assert!(!resumed.inherited.is_empty());
        assert!(resumed.accumulated > 0.);
    }
}
