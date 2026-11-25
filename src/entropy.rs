#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Genesis,
    Formation,
    Identity,
    Distortion,
    Dissolution,
    Rebirth,
}

impl Phase {
    pub const ALL: [Self; 6] = [
        Self::Genesis,
        Self::Formation,
        Self::Identity,
        Self::Distortion,
        Self::Dissolution,
        Self::Rebirth,
    ];

    pub fn title(self) -> &'static str {
        [
            "GENESIS",
            "FORMATION",
            "IDENTITY",
            "DISTORTION",
            "DISSOLUTION",
            "REBIRTH",
        ][self as usize]
    }

    pub fn roman(self) -> &'static str {
        ["I", "II", "III", "IV", "V", "VI"][self as usize]
    }
}

pub fn smooth(a: f32, b: f32, v: f32) -> f32 {
    let t = ((v - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    pub phase: Phase,
    pub cycle: u32,
    pub emergence: f32,
    pub semantic: f32,
    pub fracture: f32,
    pub entropy: f32,
    pub recall: f32,
    pub rebirth: f32,
}

impl Atmosphere {
    pub fn at(clock: f64, accumulated: f32) -> Self {
        const PERIOD: f32 = 254.;
        let revolution = (clock / f64::from(PERIOD)).floor() as u32;
        let t = (clock % f64::from(PERIOD)) as f32;
        let index = [7., 22., 100., 166., 213.].partition_point(|&boundary| t >= boundary);
        let seedling = smooth(0., 25., t);
        let rebirth = smooth(210., 243., t);
        let disintegration = smooth(100., 206., t) * (1. - rebirth);
        Self {
            phase: Phase::ALL[index],
            cycle: revolution,
            emergence: if revolution == 0 { seedling } else { 1. },
            semantic: smooth(4., 24., t)
                * (1. - smooth(106., 205., t))
                * (1. - smooth(78., 99., t) * 0.68)
                + rebirth * (1. - smooth(247., PERIOD, t)),
            fracture: disintegration,
            entropy: (0.12 + 0.33 * smooth(5., 125., t) + 0.43 * disintegration)
                * (1. - (1. - 0.12 / 0.45) * rebirth)
                + 0.16 * smooth(0., 3.5, accumulated),
            recall: smooth(151., 197., t) * (1. - smooth(226., 254., t)),
            rebirth,
        }
    }
}
