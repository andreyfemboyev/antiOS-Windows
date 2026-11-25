use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Trace {
    pub at: f32,
    pub position: Vec2,
    pub pressure: f32,
}

pub struct Cursor {
    pub position: Vec2,
    previous: Vec2,
    target: Option<Vec2>,
    pub velocity: Vec2,
    pub pressure: f32,
    pub idle: f32,
    pub held: bool,
    pub inside: bool,
    pub trail: VecDeque<Trace>,
    impulse: f32,
    last_trace: f32,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            position: Vec2::splat(0.5),
            previous: Vec2::splat(0.5),
            target: None,
            velocity: Vec2::ZERO,
            pressure: 0.,
            idle: 20.,
            held: false,
            inside: false,
            trail: VecDeque::new(),
            impulse: 0.,
            last_trace: -1.,
        }
    }
}

impl Cursor {
    pub fn move_to(&mut self, position: Vec2) {
        if !self.inside {
            self.position = position;
            self.previous = position;
        }
        self.inside = true;
        self.target = Some(position);
    }

    pub fn leave(&mut self) {
        self.inside = false;
        self.held = false;
        self.target = None;
    }

    pub fn press(&mut self, down: bool) {
        self.held = down && self.inside;
        if self.held {
            self.impulse = 1.;
        }
    }

    pub fn step(&mut self, time: f32, dt: f32) -> Option<Trace> {
        if let Some(target) = self.target {
            self.position = self.position.lerp(target, 1. - (-24. * dt).exp());
        }
        let movement = (self.position - self.previous) / dt;
        self.previous = self.position;
        self.velocity = self.velocity.lerp(movement, 1. - (-13. * dt).exp());
        self.impulse *= (-3.2 * dt).exp();
        let energy = (self.velocity.length() * 1.65 + self.impulse).min(1.);
        self.pressure += (energy - self.pressure) * (1. - (-9. * dt).exp());
        self.idle = if self.inside && (energy > 0.07 || self.held) {
            0.
        } else {
            self.idle + dt
        };
        while self.trail.front().is_some_and(|entry| time - entry.at > 8.) {
            self.trail.pop_front();
        }
        if self.inside && energy > 0.11 && time - self.last_trace >= 0.072 {
            self.last_trace = time;
            let trace = Trace {
                at: time,
                position: self.position,
                pressure: self.pressure,
            };
            self.trail.push_back(trace);
            return Some(trace);
        }
        None
    }
}
