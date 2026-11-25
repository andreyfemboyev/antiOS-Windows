use crate::{
    entropy::{smooth, Atmosphere, Phase},
    interaction::{Cursor, Trace},
    organisms::{Kind, Organism},
};
use glam::Vec2;
use serde::{Deserialize, Serialize};

pub const STEP: f32 = 1. / 60.;
const MAX_PARTICLES: usize = 680;
pub const DEFAULT_SEED: u64 = 0xA071_0506;

#[derive(Clone)]
pub struct Generator(u64);

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut n = self.0;
        n = (n ^ (n >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        n ^ (n >> 31)
    }
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / 16_777_216.
    }
    pub fn axis(&mut self) -> Vec2 {
        let a = self.unit() * std::f32::consts::TAU;
        Vec2::new(a.cos(), a.sin())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec2,
    velocity: Vec2,
    pub life: f32,
    pub duration: f32,
    pub size: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Impression {
    pub at: f32,
    pub entropy: f32,
    pub centers: [Vec2; 7],
}

pub struct World {
    pub seed: u64,
    pub frame: u64,
    pub atmosphere: Atmosphere,
    pub organisms: [Organism; 7],
    pub cursor: Cursor,
    pub particles: Vec<Particle>,
    pub impressions: Vec<Impression>,
    pub traces: Vec<Trace>,
    pub inherited: Vec<Impression>,
    pub accumulated: f32,
    pub sound_event: f32,
    pub coupled: f32,
    pub variation: f32,
    rng: Generator,
    clock: f64,
    shatter_delay: f32,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut rng = Generator::new(seed);
        let organisms =
            Kind::ALL.map(|kind| Organism::new(kind, rng.unit() * std::f32::consts::TAU));
        let variation = rng.unit();
        Self {
            seed,
            frame: 0,
            atmosphere: Atmosphere::at(0., 0.),
            organisms,
            cursor: Cursor::default(),
            particles: Vec::new(),
            impressions: Vec::new(),
            traces: Vec::new(),
            inherited: Vec::new(),
            accumulated: 0.,
            sound_event: 0.,
            coupled: 0.,
            variation,
            rng,
            clock: 0.,
            shatter_delay: 0.,
        }
    }

    pub fn time(&self) -> f32 {
        self.frame as f32 * STEP
    }

    pub fn imprint(&self) -> Impression {
        Impression {
            at: self.time(),
            entropy: self.atmosphere.entropy,
            centers: self.organisms.each_ref().map(|body| body.position),
        }
    }

    pub fn step(&mut self) {
        self.frame += 1;
        let time = self.time();
        if let Some(trace) = self.cursor.step(time, STEP) {
            if trace.pressure > 0.42 && self.traces.last().is_none_or(|old| time - old.at > 0.48) {
                if self.traces.len() == 64 {
                    self.traces.remove(0);
                }
                self.traces.push(trace);
            }
        }
        let absence = smooth(2., 14., self.cursor.idle);
        self.clock += f64::from(STEP * (0.9 + 0.10 * absence + self.cursor.pressure * 0.55));
        let old = self.atmosphere;
        self.atmosphere = Atmosphere::at(self.clock, self.accumulated);
        self.accumulated += self.atmosphere.entropy * STEP / 150.;
        self.sound_event *= 0.968;
        self.shatter_delay -= STEP;

        let positions = self.organisms.each_ref().map(|body| body.position);
        let echo = self
            .cursor
            .trail
            .iter()
            .rev()
            .find(|trace| time - trace.at > 1.3)
            .copied();
        let link = positions[Kind::Archive as usize].distance(positions[Kind::Echo as usize]);
        self.coupled += (1. - smooth(0.14, 0.28, link) - self.coupled) * 0.025;
        let fracture = self.atmosphere.fracture;
        for (index, body) in self.organisms.iter_mut().enumerate() {
            body.age += STEP;
            let drift = Vec2::new(
                (time * 0.23 + body.phase).sin(),
                (time * 0.17 + body.phase).cos(),
            );
            let mut force = (body.kind.anchor() - body.position) * (0.66 + body.stability * 0.58);
            force += drift * (0.014 + fracture * 0.038 + absence * 0.014);
            let cursor_offset = self.cursor.position - body.position;
            let radius = cursor_offset.length();
            if self.cursor.inside {
                let polarity = if self.cursor.held { -1. } else { 1. };
                force += cursor_offset.normalize_or_zero()
                    * (1. - smooth(0.018, 0.31, radius))
                    * body.curiosity
                    * polarity
                    * (0.016 + self.cursor.pressure * 0.29);
            }
            for (neighbor, point) in positions.iter().enumerate() {
                if index == neighbor {
                    continue;
                }
                let delta = *point - body.position;
                let distance = delta.length();
                let attraction = (1. - smooth(0.08, 0.32, distance)) * body.affinity * 0.057;
                let repulsion = (1. - smooth(0.0, 0.055, distance)) * 0.17;
                force += delta.normalize_or_zero() * (attraction - repulsion);
            }
            if body.kind == Kind::Echo {
                force += (positions[Kind::Archive as usize] - body.position) * self.coupled * 0.34;
                if let Some(trace) = echo {
                    force += (trace.position - body.position) * trace.pressure * 0.28;
                }
            }
            if body.kind == Kind::Unknown {
                force += Vec2::new(drift.y.sin(), (time * 0.093 + 5. * drift.x).cos()) * 0.07;
            }
            if body.kind == Kind::Fragment
                && self.cursor.inside
                && radius < 0.18
                && self.cursor.pressure > 0.35
            {
                body.stability = (body.stability - STEP * self.cursor.pressure * 1.8).max(0.);
                self.sound_event = self.sound_event.max(0.35);
            } else if body.kind == Kind::Fragment {
                body.stability = (body.stability + STEP * 0.047).min(0.44);
            }
            body.entropy +=
                (self.atmosphere.entropy * (1.3 - body.stability * 0.55) - body.entropy) * 0.012;
            let disturbed = self.cursor.inside
                && radius < 0.24
                && (self.cursor.pressure > 0.26 || self.cursor.held);
            let dormant = body.kind == Kind::Monument
                && body.velocity == Vec2::ZERO
                && fracture < 0.27
                && !disturbed;
            if !dormant {
                body.velocity = (body.velocity + force * (STEP / body.mass.sqrt())) * 0.977;
                body.position = (body.position + body.velocity * STEP)
                    .clamp(Vec2::splat(0.08), Vec2::splat(0.93));
            }
            if self.frame % 36 == 0 {
                body.memory.rotate_right(1);
                body.memory[0] = body.position;
            }
        }

        let fragment = &self.organisms[Kind::Fragment as usize];
        if self.shatter_delay <= 0.
            && (fragment.stability < 0.15
                || (old.phase != self.atmosphere.phase
                    && self.atmosphere.phase == Phase::Dissolution))
        {
            self.sound_event = 1.;
            self.shatter_delay = 4.;
            self.scatter(fragment.position, 54, 0.13);
        }
        if self.frame % 4 == 0 && self.atmosphere.emergence > 0.08 {
            let node = if self.rng.unit() > 0.55 {
                Kind::Archive
            } else {
                Kind::Unknown
            };
            let origin = self.organisms[node as usize].position
                + self.rng.axis() * (0.08 + 0.15 * self.rng.unit());
            self.scatter(origin, 1, 0.032 + fracture * 0.035);
        }
        let sink = self.organisms[Kind::Void as usize].position;
        for particle in &mut self.particles {
            let delta = sink - particle.position;
            let d = delta.length();
            particle.velocity += delta.normalize_or_zero() * (1. - smooth(0.005, 0.15, d)) * 0.0009;
            let away = particle.position - self.cursor.position;
            if self.cursor.inside {
                particle.velocity += away.normalize_or_zero()
                    * (1. - smooth(0., 0.13, away.length()))
                    * self.cursor.pressure
                    * 0.0019;
            }
            particle.velocity *= 0.996;
            particle.position += particle.velocity * STEP;
            particle.life -= STEP * if d < 0.023 { 12. } else { 1. };
        }
        self.particles.retain(|p| p.life > 0.);
        if self.frame % 720 == 0 || (old.phase != self.atmosphere.phase) {
            if self.impressions.len() == 24 {
                self.impressions.remove(0);
            }
            self.impressions.push(self.imprint());
        }
    }

    fn scatter(&mut self, center: Vec2, count: usize, speed: f32) {
        for _ in 0..count.min(MAX_PARTICLES - self.particles.len()) {
            let velocity = self.rng.axis() * speed * (0.1 + self.rng.unit());
            let duration = 3. + self.rng.unit() * 9.;
            self.particles.push(Particle {
                position: center,
                velocity,
                life: duration,
                duration,
                size: 0.55 + self.rng.unit() * 1.45,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_and_trace_reproduce_exact_state() {
        let [mut a, mut b] = [World::new(51), World::new(51)];
        for i in 0..3600 {
            for w in [&mut a, &mut b] {
                if i < 300 {
                    w.cursor.move_to(Vec2::new(0.55 + i as f32 * 0.001, 0.66));
                }
                if i == 240 {
                    w.cursor.press(true);
                }
                if i == 480 {
                    w.cursor.leave();
                }
                w.step();
            }
        }
        assert_eq!(a.organisms, b.organisms);
        assert_eq!(a.particles, b.particles);
        assert_eq!(a.accumulated, b.accumulated);
        assert!(!a.traces.is_empty());
        assert_eq!(a.atmosphere.phase, Phase::Identity);
    }

    #[test]
    fn monument_waits_for_disturbance() {
        let mut world = World::new(DEFAULT_SEED);
        let index = Kind::Monument as usize;
        for _ in 0..1200 {
            world.step();
        }
        let rest = world.organisms[index].position;
        assert_eq!(rest, Kind::Monument.anchor());
        world.cursor.move_to(rest + Vec2::new(0.09, -0.03));
        world.cursor.press(true);
        for _ in 0..90 {
            world.step();
        }
        assert_ne!(world.organisms[index].position, rest);
    }

    #[test]
    fn autonomous_world_cycles_and_remains_bounded() {
        let mut w = World::new(13);
        let mut seen = [false; 6];
        for _ in 0..16_500 {
            seen[w.atmosphere.phase as usize] = true;
            w.step();
        }
        assert!(seen.into_iter().all(|phase| phase));
        assert!(w.organisms.iter().all(|o| o.position.is_finite()));
        assert!(w.particles.len() < MAX_PARTICLES);
        assert_eq!(w.atmosphere.cycle, 1);
    }
}
