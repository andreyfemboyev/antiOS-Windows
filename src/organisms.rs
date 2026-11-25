use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Kind {
    Observer,
    Archive,
    Void,
    Echo,
    Fragment,
    Monument,
    Unknown,
}

impl Kind {
    pub const ALL: [Self; 7] = [
        Self::Observer,
        Self::Archive,
        Self::Void,
        Self::Echo,
        Self::Fragment,
        Self::Monument,
        Self::Unknown,
    ];
    pub fn anchor(self) -> Vec2 {
        match self {
            Self::Observer => Vec2::new(0.795, 0.29),
            Self::Archive => Vec2::new(0.711, 0.505),
            Self::Void => Vec2::new(0.881, 0.548),
            Self::Echo => Vec2::new(0.663, 0.673),
            Self::Fragment => Vec2::new(0.902, 0.687),
            Self::Monument => Vec2::new(0.21, 0.48),
            Self::Unknown => Vec2::new(0.532, 0.292),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Organism {
    pub kind: Kind,
    pub position: Vec2,
    pub velocity: Vec2,
    pub age: f32,
    pub memory: [Vec2; 12],
    pub mass: f32,
    pub stability: f32,
    pub curiosity: f32,
    pub entropy: f32,
    pub affinity: f32,
    pub phase: f32,
}

impl Organism {
    pub fn new(kind: Kind, phase: f32) -> Self {
        let (mass, stability, curiosity, affinity) = match kind {
            Kind::Observer => (1., 0.76, 0.85, 0.28),
            Kind::Archive => (3.2, 0.95, 0.22, 0.42),
            Kind::Void => (2.8, 0.91, -0.24, -0.32),
            Kind::Echo => (0.8, 0.62, 0.62, 0.47),
            Kind::Fragment => (0.52, 0.36, -0.86, 0.21),
            Kind::Monument => (6., 0.99, -0.06, 0.12),
            Kind::Unknown => (1.2, 0.65, 0.29, 0.33),
        };
        let position = kind.anchor();
        Self {
            kind,
            position,
            velocity: Vec2::ZERO,
            age: 0.,
            memory: [position; 12],
            mass,
            stability,
            curiosity,
            entropy: 0.,
            affinity,
            phase,
        }
    }
}
