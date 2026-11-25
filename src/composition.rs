use crate::{
    configuration::{Preferences, Quality},
    entropy::smooth,
    organisms::Kind,
    renderer::{Color, Frame, Layer, CANVAS, MUTED, PAPER, SIGNAL, SURFACE, VOID},
    simulation::{Generator, World, DEFAULT_SEED},
    typography::{Fragmentation, Type, Typography},
};
use glam::{vec2, vec3, Vec2, Vec3};
use std::f32::consts::TAU;

const INK: Color = Color::rgb(23, 23, 20);
const BLIND: Color = Color::rgb(63, 63, 60);
const DIM: Color = Color::rgb(157, 155, 149);
const RULE: Color = Color::rgb(55, 55, 51);
const OVERLAY_BORDER: Color = Color::rgb(98, 98, 90);

const PHRASES: [[&str; 3]; 7] = [
    ["ABSENCE", "IS AN", "INPUT."],
    ["NOTHING", "HAS", "FAILED."],
    ["THERE IS", "NO", "DESKTOP."],
    ["CONTROL", "IS AN", "ILLUSION."],
    ["PURPOSE", "IS", "OPTIONAL."],
    ["A WINDOW", "WITHOUT", "A WORLD."],
    ["THE MACHINE", "REMEMBERS", "NOTHING."],
];

pub fn frame(world: &World, reduced: bool) -> Frame {
    Frame {
        time: world.time(),
        fracture: world.atmosphere.fracture * if reduced { 0.19 } else { 1. },
        persistence: if reduced {
            0.14
        } else {
            0.34 + world.atmosphere.recall * 0.23
        },
    }
}

pub fn artwork(
    layer: &mut Layer,
    typeface: &Typography,
    world: &World,
    quality: Quality,
    reduced: bool,
) {
    layer.clear();
    let a = world.atmosphere;
    let time = world.time();
    let motion = if reduced { 0.18 } else { 1. };
    let wake = a.emergence;
    let mut rng = Generator::new(world.seed ^ 713);
    for row in 0..17 {
        for col in 0..36 {
            let x = 71. + col as f32 * 37.;
            let y = 151. + row as f32 * 36.5;
            let opacity = (0.06 + rng.unit() * 0.21) * (0.29 + wake * 0.71);
            layer.dot(
                vec2(x, y),
                if rng.unit() > 0.95 { 0.95 } else { 0.53 },
                MUTED.alpha(opacity),
            );
        }
    }

    coordinate_field(layer, world, motion);
    let print = a.emergence * (1. - 0.74 * a.fracture);
    plate(layer, typeface, world, quality, print, motion);
    organisms(layer, typeface, world, motion);
    let initial_phrase = if world.seed == DEFAULT_SEED {
        0
    } else {
        (world.seed as usize) % PHRASES.len()
    };
    let semantic = (a.semantic * (1. - a.fracture * 0.26) + a.fracture * 0.25).min(1.);
    for (transition, weight) in [1. - a.rebirth, a.rebirth].into_iter().enumerate() {
        if semantic * weight <= 0.004 {
            continue;
        }
        let phrase_index = (initial_phrase + a.cycle as usize + transition) % PHRASES.len();
        let center = world.organisms[Kind::Monument as usize].position;
        let movement = (center - Kind::Monument.anchor()) * vec2(124., 97.) * motion;
        let baseline = vec2(109., 371.) + movement;
        let fragmentation = Fragmentation {
            time,
            entropy: a.fracture * (0.7 + a.entropy * 0.3),
            phase: world.variation * TAU,
            reduced,
        };
        let widest = PHRASES[phrase_index]
            .iter()
            .map(|line| typeface.width(line, Type::display(133., -7.4, PAPER)))
            .fold(1., f32::max);
        let size = 133. * (655. / widest).min(1.);
        for (i, line) in PHRASES[phrase_index].iter().enumerate() {
            let opacity = semantic * weight * (1. - 0.08 * i as f32 * a.fracture);
            typeface.fractured(
                layer,
                line,
                baseline + vec2(0., 115. * i as f32),
                Type::display(size, -size * 0.055, PAPER.alpha(opacity)),
                &fragmentation,
            );
        }
    }
    impulses(layer, world, motion);
}

fn plate(
    layer: &mut Layer,
    typeface: &Typography,
    world: &World,
    quality: Quality,
    density: f32,
    motion: f32,
) {
    if density < 0.012 {
        return;
    }
    let archive = &world.organisms[Kind::Archive as usize];
    let learned = archive.memory[5] - archive.position;
    let position =
        vec2(811., 161.) + (archive.position - Kind::Archive.anchor()) * vec2(250., 164.) * motion;
    let fracture = world.atmosphere.fracture;
    let size = vec2(441., 571.);
    let paper_opacity = smooth(0.02, 0.78, density);
    let print_index = format!("{:02}", world.atmosphere.cycle % 99 + 1);
    if world.coupled > 0.05 {
        let bridge = world.coupled * density;
        layer.outline(
            position + vec2(-14., -13.),
            size + vec2(28., 26.),
            0.8,
            MUTED.alpha(bridge * 0.57),
        );
        layer.outline(
            position + vec2(-22., -21.) + learned * 34.,
            size + vec2(44., 42.),
            0.65,
            MUTED.alpha(bridge * 0.42),
        );
    }
    layer.rect(
        position,
        size,
        PAPER.alpha(paper_opacity * (1. - fracture * 0.34)),
    );

    let pigment = INK.alpha(paper_opacity);
    let register = INK.alpha(paper_opacity * 0.44);
    let inset = 19.;
    layer.line(
        position + vec2(inset, 60.),
        position + vec2(size.x - inset, 60.),
        0.8,
        register,
    );
    layer.line(
        position + vec2(inset, size.y - 49.),
        position + vec2(size.x - inset, size.y - 49.),
        0.8,
        register,
    );
    typeface.text(
        layer,
        "Archive",
        position + vec2(inset, 39.),
        Type::display(25., -0.8, pigment),
    );
    typeface.text(
        layer,
        &print_index,
        position + vec2(size.x - 51., 36.),
        Type::mono(15., 1.6, pigment),
    );
    typeface.text(
        layer,
        "RESIDUAL FORM",
        position + vec2(inset, size.y - 23.),
        Type::mono(12., 1.1, pigment),
    );
    typeface.text(
        layer,
        "FIG. A",
        position + vec2(size.x - 70., size.y - 23.),
        Type::mono(12., 0.8, pigment),
    );

    let center = position + vec2(209., 284.);
    let height = 428.;
    for i in 1..14 {
        let y = position.y + 75. + height * i as f32 / 14.;
        for x in [position.x + 22., position.x + size.x - 22.] {
            layer.line(vec2(x - 2., y), vec2(x + 2., y), 0.66, register.alpha(0.58));
        }
    }
    for i in 0..120 {
        let mut rng = Generator::new(world.seed.wrapping_add(i * 611));
        let x = position.x + 32. + rng.unit() * 376.;
        let y = position.y + 73. + rng.unit() * 437.;
        layer.dot(
            vec2(x, y),
            0.22 + rng.unit() * 0.62,
            register.alpha(0.15 + rng.unit() * 0.38),
        );
    }

    let detail = match quality {
        Quality::Low => (43, 86, 8),
        Quality::Medium => (63, 111, 12),
        _ => (84, 135, 17),
    };
    let time = world.time() * motion;
    let seed_phase = if world.seed == DEFAULT_SEED {
        0.62
    } else {
        0.15 + world.variation * 1.26
    };
    let orientation = 0.25 + (time * 0.085 + seed_phase).sin() * 0.12 + fracture * 0.33;
    let influence = learned * 8. + world.cursor.velocity * world.cursor.pressure * 0.24 * motion;
    let body = Engraving {
        center,
        seed_phase,
        orientation,
        influence,
        bend: (time * 0.071).sin() * 0.105 + world.coupled * 0.21,
        wound: fracture * 0.69 + archive.entropy * 0.22,
    };
    let line_ink = INK.alpha(paper_opacity * (1. - fracture * 0.57));
    for row in 0..detail.0 {
        let v = row as f32 * TAU / detail.0 as f32;
        let mut previous = body.project(0., v);
        for col in 1..=detail.1 {
            let u = col as f32 * TAU / detail.1 as f32;
            let next = body.project(u, v);
            let occlusion = smooth(-0.59, 0.75, (previous.z + next.z) * 0.5);
            let opening = 1. - fracture * smooth(0.36, 0.77, (0.7 * u + v * 1.3).sin());
            let ink = (0.23 + occlusion * 0.64) * opening;
            let width = 0.57 + occlusion * 0.48 + 0.16 * (v * 3. + seed_phase).cos();
            layer.line(
                previous.truncate(),
                next.truncate(),
                width,
                line_ink.alpha(ink),
            );
            previous = next;
        }
    }
    for row in 0..detail.2 {
        let u = (row as f32 + 0.34) * TAU / detail.2 as f32;
        let mut previous = body.project(u, 0.);
        for step in 1..=41 {
            let point = body.project(u, step as f32 * TAU / 41.);
            layer.line(
                previous.truncate(),
                point.truncate(),
                0.92,
                line_ink.alpha(0.28 + 0.38 * smooth(-0.15, 0.8, point.z)),
            );
            previous = point;
        }
    }
    // The plotter's datum crosses the living figure only where it reaches the paper margin.
    for shift in [-170., 147.] {
        cross(layer, center + vec2(shift, 25.), 4.1, 0.7, register);
    }
}

struct Engraving {
    center: Vec2,
    seed_phase: f32,
    orientation: f32,
    influence: Vec2,
    bend: f32,
    wound: f32,
}

impl Engraving {
    fn project(&self, u: f32, v: f32) -> Vec3 {
        let (sin_u, cos_u) = u.sin_cos();
        let angular = v + 0.76 * sin_u + self.bend * (2. * u + self.seed_phase).sin();
        let (sin_v, cos_v) = angular.sin_cos();
        let knots = 0.19 * (2. * u - self.seed_phase).cos() + 0.11 * (3. * u + 0.7).sin();
        let tube = (0.46 + 0.105 * (u + 1.5).sin() + 0.07 * (3. * u + self.seed_phase).cos())
            * (1. + 0.16 * (3. * v - u).cos() + self.wound * 0.39 * (5. * u - 2. * v).sin());
        let radius = 0.76 + knots + tube * cos_v;
        let x = radius * cos_u + 0.11 * (v - 2. * u).sin();
        let z = radius * sin_u + 0.31 * (2. * u + self.seed_phase).sin();
        let y = tube * sin_v * 0.84
            + 0.42 * (u + 0.8).sin()
            + 0.23 * (2. * u - self.seed_phase).cos()
            + self.bend * (u - v).sin();
        let incline: f32 = 0.97 + self.wound * 0.23;
        let (s, c) = self.orientation.sin_cos();
        let vertical = y * incline.cos() + z * incline.sin();
        let depth = z * incline.cos() - y * incline.sin();
        let projection = 1. + 0.13 * depth;
        let screen = self.center
            + vec2(c * x - s * vertical, s * x + c * vertical) * 133. * projection
            + self.influence * (1. + depth) * 4.;
        vec3(screen.x, screen.y, depth)
    }
}

fn coordinate_field(layer: &mut Layer, world: &World, motion: f32) {
    let a = world.atmosphere;
    let alpha = 0.26 + 0.74 * a.emergence;
    let x = 768.;
    let p = vec2(x, 451.);
    layer.line(vec2(x, 130.), vec2(x, 770.), 0.8, RULE.alpha(0.66 * alpha));
    for i in 0..22 {
        let y = 138. + i as f32 * 29.8;
        layer.line(
            vec2(x - 2., y),
            vec2(x + if i % 5 == 0 { 8. } else { 3. }, y),
            0.75,
            MUTED.alpha(0.53 * alpha),
        );
    }
    let mover = world.organisms[Kind::Unknown as usize].position * CANVAS;
    let center = p.lerp(mover, 0.09 * motion) + vec2(29., -31.);
    for (radius, rotation, weight) in [
        (vec2(537., 191.), -0.45, 0.38),
        (vec2(463., 208.), -0.55, 0.24),
    ] {
        layer.ring(center, radius, rotation, 1., BLIND.alpha(weight * alpha));
    }
    for (x, y) in [
        (81., 156.),
        (1359., 156.),
        (81., 750.),
        (1359., 750.),
        (753., 361.),
        (1318., 442.),
    ] {
        cross(layer, vec2(x, y), 4., 0.8, MUTED.alpha(0.45 * alpha));
    }
    if a.emergence < 0.68 {
        let alpha = 1. - smooth(0.02, 0.68, a.emergence);
        layer.ring(p, vec2(81., 81.), 0., 0.9, MUTED.alpha(alpha * 0.7));
        layer.ring(p, vec2(27., 27.), 0., 0.8, MUTED.alpha(alpha * 0.65));
        layer.dot(p, 3.2, SIGNAL.alpha(alpha));
    }
}

fn organisms(layer: &mut Layer, face: &Typography, world: &World, motion: f32) {
    let a = world.atmosphere;
    let now = world.time();
    let waking = a.emergence;
    let color = MUTED.alpha(waking * (0.34 + a.entropy * 0.32));
    for body in &world.organisms {
        let delta = body.position - body.kind.anchor();
        let p = body.kind.anchor() * CANVAS + delta * CANVAS * motion;
        match body.kind {
            Kind::Observer => {
                let center = p + vec2(178., -40.);
                let follow = (world.cursor.position * CANVAS - center).normalize_or_zero()
                    * world.cursor.pressure
                    * 6.
                    * motion;
                layer.outline(center - vec2(57., 35.), vec2(114., 70.), 0.8, color);
                layer.ring(center + follow, vec2(19., 19.), 0., 0.8, color);
                cross(layer, center + follow, 5.3, 0.8, color);
                layer.dot(center + vec2(49., -27.), 1.1, PAPER.alpha(waking * 0.49));
            }
            Kind::Archive => {
                for (index, point) in body.memory.iter().enumerate().skip(3).step_by(3) {
                    let d = (*point - body.position) * CANVAS * motion * 0.7;
                    let size = vec2(468., 601.) + vec2(13., 10.) * (index as f32 / 3.);
                    let center = vec2(1030., 444.) + d;
                    let tint = color.alpha(0.52 / (index as f32 * 0.29) + a.recall * 0.29);
                    layer.outline(center - size * 0.5, size, 0.7, tint);
                }
            }
            Kind::Void => {
                let center = p + vec2(50., -7.);
                let radius = (13. + body.entropy * 12.) * waking;
                layer.dot(center, radius, VOID.alpha(0.92));
                layer.ring(
                    center,
                    Vec2::splat(radius + 2.6),
                    0.,
                    0.7,
                    color.alpha(0.72),
                );
                for i in 0..7 {
                    let arc = i as f32 * 0.39 + now * 0.17 * motion;
                    layer.dot(
                        center + vec2(arc.cos(), arc.sin()) * (radius + 5. + i as f32 * 1.7),
                        0.6,
                        color.alpha(0.8),
                    );
                }
            }
            Kind::Echo => {
                let position = p + vec2(-262., 44.);
                for index in 0..9 {
                    let d = vec2(index as f32 * 5.5, index as f32 * 2.7);
                    layer.line(
                        position + d,
                        position + d + vec2(38. - index as f32 * 1.7, 0.),
                        0.75,
                        color.alpha(0.34 + (8 - index) as f32 * 0.05),
                    );
                }
            }
            Kind::Fragment => {
                let center = p + vec2(6., 14.);
                let stability = (1. - body.stability) * (8. + a.fracture * 42.) * motion;
                for i in 0..10 {
                    let theta = body.phase + i as f32 * 2.39;
                    let d = vec2(theta.cos(), theta.sin()) * (10. + i as f32 * 3. + stability);
                    let tint = color.alpha(1. - i as f32 * 0.05);
                    layer.line(
                        center + d,
                        center + d + vec2(8. + i as f32 * 0.4, (theta * 2.).sin() * 4.),
                        0.8 + 0.75 * body.entropy,
                        tint,
                    );
                }
            }
            Kind::Monument => {
                let d = delta * vec2(97., 58.) * motion;
                let from = vec2(110., 631.) + d;
                layer.line(from, from + vec2(94., 0.), 0.8, color);
            }
            Kind::Unknown => {
                let c = p + vec2(-24., -46.);
                let tilt = (now * 0.21 * motion + body.phase).sin() * 0.49;
                for i in 0..3 {
                    let radius = 12. + i as f32 * 6.;
                    layer.ring(
                        c,
                        vec2(radius * 1.2, radius * 0.48),
                        tilt + i as f32 * 0.64,
                        0.8,
                        color,
                    );
                }
                if waking > 0.35 {
                    face.text(layer, "?", c + vec2(-5., -29.), Type::mono(11., 0., color));
                }
            }
        }
    }
    let historical = if world.inherited.is_empty() {
        &world.impressions
    } else {
        &world.inherited
    };
    for (age, impression) in historical.iter().rev().take(7).enumerate() {
        let pigment = MUTED.alpha((a.recall * 0.44 + waking * 0.15) / (1. + 0.72 * age as f32));
        for centers in impression.centers.windows(2) {
            let from = centers[0] * CANVAS;
            let to = centers[1] * CANVAS;
            let length = from.distance(to);
            let offset = vec2(age as f32 * 7., -(age as f32) * 5.);
            let distort = 12. + impression.entropy * 36. + impression.at * 0.023;
            layer.ring(
                (from + to) / 2. + offset,
                vec2(length * 0.5 + 8., distort),
                (to - from).to_angle(),
                0.8,
                pigment,
            );
        }
    }
    for particle in &world.particles {
        let p = particle.position * CANVAS;
        let brightness = smooth(0., 0.9, particle.life)
            * (1. - smooth(0.78, 1., particle.life / particle.duration));
        let ink = if p.x > 822. && p.x < 1243. && p.y > 169. && p.y < 726. && waking > 0.6 {
            INK
        } else {
            PAPER
        };
        layer.dot(
            p,
            particle.size * 0.68,
            ink.alpha(brightness * (0.2 + 0.38 * a.fracture) * motion),
        );
    }
}

fn impulses(layer: &mut Layer, world: &World, motion: f32) {
    let time = world.time();
    let mut last: Option<(Vec2, f32)> = None;
    for trace in &world.cursor.trail {
        let age = time - trace.at;
        let p = trace.position * CANVAS;
        let on_plate = p.x > 818.
            && p.x < 1249.
            && p.y > 161.
            && p.y < 732.
            && world.atmosphere.emergence > 0.5;
        let ink = if on_plate { INK } else { PAPER };
        let strength = trace.pressure * (1. - smooth(0.2, 6.8, age)) * motion;
        if let Some((previous, at)) = last {
            if trace.at - at < 0.28 {
                layer.line(
                    previous,
                    p,
                    0.58 + trace.pressure * 0.65,
                    ink.alpha(strength * 0.62),
                );
            }
        }
        let echo = smooth(1.2, 1.36, age) * (1. - smooth(1.37, 3.0, age)) * trace.pressure;
        if echo > 0.08 {
            layer.ring(
                p,
                Vec2::splat(4. + 15. * (age - 1.2)),
                0.,
                0.86,
                ink.alpha(echo * 0.38 * motion),
            );
        }
        last = Some((p, trace.at));
    }
    if world.cursor.inside && (world.cursor.held || world.cursor.pressure > 0.3) {
        let p = world.cursor.position * CANVAS;
        layer.ring(
            p,
            Vec2::splat(6. + world.cursor.pressure * 16. * motion),
            0.,
            0.8,
            SIGNAL.alpha(0.36 + world.cursor.pressure * 0.62),
        );
    }
}

fn cross(layer: &mut Layer, p: Vec2, size: f32, weight: f32, ink: Color) {
    layer.line(p - Vec2::X * size, p + Vec2::X * size, weight, ink);
    layer.line(p - Vec2::Y * size, p + Vec2::Y * size, weight, ink);
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Dialog {
    #[default]
    None,
    Controls,
    Reset,
    Exit,
}

#[derive(Clone, Copy)]
pub enum Action {
    Pause,
    Mute,
    Fullscreen,
    Capture,
    Motion,
    Quality,
    Volume(f32),
    Reset,
    Exit,
    Confirm,
    Back,
}

pub struct Interface<'a> {
    pub dialog: Dialog,
    pub paused: bool,
    pub muted: bool,
    pub fullscreen: bool,
    pub preferences: &'a Preferences,
    pub status: Option<&'a str>,
}

pub const CONTROL_ORIGIN: Vec2 = Vec2::new(912., 316.);

fn button(face: &Typography, layer: &mut Layer, p: Vec2, key: &str, title: &str, value: &str) {
    layer.outline(p + vec2(0., -17.), vec2(42., 24.), 0.8, RULE);
    face.text(
        layer,
        key,
        p + vec2(21. - face.width(key, Type::mono(11., 0.4, PAPER)) / 2., 0.),
        Type::mono(11., 0.4, PAPER),
    );
    face.text(layer, title, p + vec2(61., 0.), Type::mono(13., 0.6, PAPER));
    let format = Type::mono(12., 0.6, DIM);
    face.text(
        layer,
        value,
        p + vec2(387. - face.width(value, format), 0.),
        format,
    );
}

pub fn hit(point: Vec2, dialog: Dialog) -> Option<Action> {
    if dialog == Dialog::None {
        return (point.x > 1230. && point.y > 802.).then_some(Action::Back);
    }
    if dialog == Dialog::Reset || dialog == Dialog::Exit {
        let within = point.y > 463. && point.y < 517.;
        return if within && (720. ..890.).contains(&point.x) {
            Some(Action::Confirm)
        } else if within && (513. ..700.).contains(&point.x) {
            Some(Action::Back)
        } else {
            None
        };
    }
    let p = point - CONTROL_ORIGIN;
    if p.x < 18. || p.x > 437. {
        return None;
    }
    if (94. ..128.).contains(&p.y) && (349. ..426.).contains(&p.x) {
        return Some(Action::Volume(((p.x - 349.) / 77.).clamp(0., 1.)));
    }
    if (58. ..305.).contains(&p.y) {
        return Some(
            [
                Action::Pause,
                Action::Mute,
                Action::Fullscreen,
                Action::Capture,
                Action::Motion,
                Action::Quality,
                Action::Reset,
                Action::Exit,
            ][((p.y - 58.) / 31.) as usize],
        );
    }
    (22. ..49.).contains(&p.y).then_some(Action::Back)
}

pub fn chrome(layer: &mut Layer, face: &Typography, world: &World, ui: &Interface<'_>) {
    layer.clear();
    let paper = PAPER;
    let grey = DIM;
    let small = Type::mono(13., 1.15, grey);
    let lead = Type::mono(13., 1.5, paper);
    let low = Type::mono(11., 1.2, MUTED);

    logo(layer, vec2(59., 44.), PAPER);
    face.text(
        layer,
        "antiOS",
        vec2(107., 73.),
        Type::display(38., -1.95, PAPER),
    );
    layer.line(vec2(234., 47.), vec2(234., 77.), 1., RULE);
    face.text(
        layer,
        "An operating system without a purpose.",
        vec2(254., 66.),
        small,
    );
    let identifier = format!("WORLD / {:08X}", world.seed as u32);
    face.text(
        layer,
        &identifier,
        vec2(1382. - face.width(&identifier, small), 66.),
        small,
    );
    layer.line(vec2(58., 105.), vec2(1382., 105.), 0.94, RULE);
    layer.outline(vec2(58., 128.), vec2(1324., 647.), 0.82, RULE.alpha(0.84));

    let phase = format!(
        "{} / {}",
        world.atmosphere.phase.roman(),
        world.atmosphere.phase.title()
    );
    layer.line(vec2(110., 228.), vec2(128., 228.), 1.7, SIGNAL.alpha(0.89));
    face.text(
        layer,
        &phase,
        vec2(139., 233.),
        Type::mono(13., 2.2, PAPER.alpha(0.88)),
    );

    layer.line(vec2(58., 800.), vec2(1382., 800.), 0.94, RULE);
    face.text(
        layer,
        if ui.paused { "PAUSED" } else { "ELAPSED" },
        vec2(58., 835.),
        low,
    );
    let seconds = world.time() as u64;
    let timecode = format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    );
    face.text(layer, &timecode, vec2(58., 858.), lead);
    face.text(layer, "ENTROPY", vec2(243., 835.), low);
    face.text(
        layer,
        &format!("{:0.3}", world.atmosphere.entropy),
        vec2(243., 858.),
        lead,
    );
    for i in 0..23 {
        let ink = if i as f32 / 23. <= world.atmosphere.entropy {
            DIM
        } else {
            RULE
        };
        layer.rect(vec2(308. + i as f32 * 3.43, 849.), vec2(1.5, 10.), ink);
    }

    face.text(
        layer,
        if ui.muted { "SOUND OFF" } else { "SOUND ON" },
        vec2(1060., 854.),
        small,
    );
    layer.outline(vec2(1226., 827.), vec2(156., 39.), 1., RULE);
    layer.outline(vec2(1237., 835.), vec2(34., 23.), 0.8, RULE);
    face.text(layer, "ESC", vec2(1244., 851.), Type::mono(10., 0.3, PAPER));
    face.text(layer, "CONTROLS", vec2(1284., 852.), lead);
    if let Some(status) = ui.status {
        face.text(
            layer,
            status,
            vec2(484., 855.),
            Type::mono(12., 0.55, PAPER),
        );
    }

    match ui.dialog {
        Dialog::None => {}
        Dialog::Controls => controls(layer, face, ui),
        Dialog::Reset | Dialog::Exit => confirm(layer, face, ui.dialog),
    }
}

fn controls(layer: &mut Layer, face: &Typography, ui: &Interface<'_>) {
    let o = CONTROL_ORIGIN;
    layer.rect(o, vec2(450., 369.), SURFACE.alpha(0.985));
    layer.outline(o, vec2(450., 369.), 1., OVERLAY_BORDER);
    face.text(
        layer,
        "Controls",
        o + vec2(28., 45.),
        Type::display(32., -0.9, PAPER),
    );
    face.text(
        layer,
        "ESC / CLOSE",
        o + vec2(327., 41.),
        Type::mono(12., 0.4, DIM),
    );
    layer.line(o + vec2(28., 57.), o + vec2(419., 57.), 0.8, RULE);
    let items = [
        (
            "SPACE",
            "Simulation",
            if ui.paused { "PAUSED" } else { "ACTIVE" },
        ),
        (
            "M",
            "Sound · volume",
            if ui.muted { "MUTED" } else { "LIVE" },
        ),
        (
            "F11",
            "Fullscreen",
            if ui.fullscreen { "ON" } else { "OFF" },
        ),
        ("S", "Capture artwork", "SHIFT+S / 4K"),
        (
            "D",
            "Reduced motion",
            if ui.preferences.reduced_motion {
                "ON"
            } else {
                "OFF"
            },
        ),
        ("G", "Rendering quality", ui.preferences.quality.name()),
        ("R", "Begin a new world", ""),
        ("Q", "Exit antiOS", ""),
    ];
    for (i, (key, title, value)) in items.into_iter().enumerate() {
        let value = if i == 1 { "" } else { value };
        button(
            face,
            layer,
            o + vec2(29., 81. + i as f32 * 31.),
            key,
            title,
            value,
        );
    }
    face.text(
        layer,
        if ui.muted { "MUTED" } else { "LIVE" },
        o + vec2(282., 112.),
        Type::mono(11., 0.4, DIM),
    );
    let base = o + vec2(349., 108.);
    layer.line(base, base + Vec2::X * 77., 1.1, MUTED);
    layer.dot(base + Vec2::X * (77. * ui.preferences.volume), 3.4, SIGNAL);
    layer.line(o + vec2(28., 316.), o + vec2(419., 316.), 0.8, RULE);
    face.text(
        layer,
        "− / +  VOLUME       HOLD CLICK  REPEL",
        o + vec2(29., 343.),
        Type::mono(11., 0.65, DIM),
    );
}

fn confirm(layer: &mut Layer, face: &Typography, dialog: Dialog) {
    layer.rect(Vec2::ZERO, CANVAS, VOID.alpha(0.65));
    let p = vec2(486., 358.);
    layer.rect(p, vec2(468., 171.), SURFACE);
    layer.outline(p, vec2(468., 171.), 1., OVERLAY_BORDER);
    let (title, explanation) = if dialog == Dialog::Reset {
        (
            "Begin a new world",
            "This world's fragments will enter the archive.",
        )
    } else {
        (
            "Exit antiOS",
            "A fragment of this world will be remembered.",
        )
    };
    face.text(
        layer,
        title,
        p + vec2(28., 44.),
        Type::display(29., -0.68, PAPER),
    );
    face.text(
        layer,
        explanation,
        p + vec2(28., 75.),
        Type::mono(13., -0.12, DIM),
    );
    button(face, layer, p + vec2(28., 130.), "ESC", "Cancel", "");
    button(face, layer, p + vec2(236., 130.), "ENTER", "Confirm", "");
    layer.dot(p + vec2(439., 29.), 2.9, SIGNAL);
}

pub fn logo(layer: &mut Layer, o: Vec2, ink: Color) {
    let p = |x, y| o + vec2(x, y);
    layer.ring(p(17., 17.), vec2(15.7, 15.7), 0., 1.55, ink);
    layer.line(p(4., 30.), p(30., 4.), 1.55, ink);
    layer.line(p(20., 4.), p(30., 4.), 1.55, ink);
    layer.line(p(30., 4.), p(30., 14.), 1.55, ink);
    layer.dot(p(6., 5.), 1.65, SIGNAL);
    let end = p(33., 29.);
    layer.line(end, end + vec2(5., -5.), 1.2, ink);
}
