use crate::renderer::{Color, Layer};
use fontdue::{Font, FontSettings, Metrics};
use glam::Vec2;
use std::collections::HashMap;

const WIDTH: usize = 2048;
const HEIGHT: usize = 2048;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Display,
    Mono,
}

impl Face {
    fn native_size(self) -> f32 {
        match self {
            Self::Display => 176.,
            Self::Mono => 42.,
        }
    }
}

#[derive(Clone, Copy)]
struct Glyph {
    metric: Metrics,
    uv: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct Type {
    pub face: Face,
    pub size: f32,
    pub tracking: f32,
    pub color: Color,
}

impl Type {
    pub const fn display(size: f32, tracking: f32, color: Color) -> Self {
        Self {
            face: Face::Display,
            size,
            tracking,
            color,
        }
    }
    pub const fn mono(size: f32, tracking: f32, color: Color) -> Self {
        Self {
            face: Face::Mono,
            size,
            tracking,
            color,
        }
    }
}

pub struct Fragmentation {
    pub time: f32,
    pub entropy: f32,
    pub phase: f32,
    pub reduced: bool,
}

pub struct Typography {
    glyphs: HashMap<(Face, char), Glyph>,
    pub atlas_size: (u32, u32),
    pub pixels: Vec<u8>,
}

impl Typography {
    pub fn new() -> Self {
        let mut pixels = vec![0; WIDTH * HEIGHT];
        let mut glyphs = HashMap::new();
        let (mut x, mut y, mut shelf) = (2, 2, 0);
        for (face, bytes) in [
            (
                Face::Display,
                &include_bytes!("../assets/Manrope-ExtraBold.ttf")[..],
            ),
            (
                Face::Mono,
                &include_bytes!("../assets/IBMPlexMono-Regular.ttf")[..],
            ),
        ] {
            let font = Font::from_bytes(
                bytes,
                FontSettings {
                    scale: face.native_size(),
                    ..Default::default()
                },
            )
            .expect("bundled fonts are valid");
            let chars = if face == Face::Mono {
                (32_u8..127).map(char::from).collect::<String>() + "×—·−"
            } else {
                " ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.-?".into()
            };
            for ch in chars.chars() {
                let (metric, bitmap) = font.rasterize(ch, face.native_size());
                if x + metric.width + 3 >= WIDTH {
                    x = 2;
                    y += shelf + 5;
                    shelf = 0;
                }
                assert!(
                    y + metric.height + 3 < HEIGHT,
                    "embedded atlas has room for the glyph set"
                );
                for (row, line) in bitmap.chunks(metric.width.max(1)).enumerate() {
                    let loc = (y + row) * WIDTH + x;
                    pixels[loc..loc + line.len()].copy_from_slice(line);
                }
                let uv = [
                    x as f32 / WIDTH as f32,
                    y as f32 / HEIGHT as f32,
                    (x + metric.width) as f32 / WIDTH as f32,
                    (y + metric.height) as f32 / HEIGHT as f32,
                ];
                glyphs.insert((face, ch), Glyph { metric, uv });
                x += metric.width + 5;
                shelf = shelf.max(metric.height);
            }
        }
        Self {
            glyphs,
            atlas_size: (WIDTH as u32, HEIGHT as u32),
            pixels,
        }
    }

    fn glyph(&self, face: Face, ch: char) -> Glyph {
        *self
            .glyphs
            .get(&(face, ch))
            .unwrap_or(&self.glyphs[&(face, '?')])
    }

    pub fn width(&self, text: &str, style: Type) -> f32 {
        let scale = style.size / style.face.native_size();
        text.chars()
            .map(|ch| self.glyph(style.face, ch).metric.advance_width * scale + style.tracking)
            .sum()
    }

    pub fn text(&self, layer: &mut Layer, text: &str, baseline: Vec2, style: Type) {
        self.compose(layer, text, baseline, style, None);
    }

    pub fn fractured(
        &self,
        layer: &mut Layer,
        text: &str,
        baseline: Vec2,
        style: Type,
        design: &Fragmentation,
    ) {
        self.compose(layer, text, baseline, style, Some(design));
    }

    fn compose(
        &self,
        layer: &mut Layer,
        text: &str,
        baseline: Vec2,
        style: Type,
        design: Option<&Fragmentation>,
    ) {
        let scale = style.size / style.face.native_size();
        let mut pen = baseline.x;
        for (index, ch) in text.chars().enumerate() {
            let mut displacement = Vec2::ZERO;
            let mut angle = 0.;
            let mut pigment = style.color;
            let mut mask = 1.;
            let mut tracking = style.tracking;
            let mut symbol = ch;
            if let Some(form) = design {
                let time = if form.reduced { 0. } else { form.time };
                let index_phase = index as f32 * 1.72 + form.phase;
                let wave = (index_phase + time * 0.21).sin();
                let disintegrate = form.entropy.powi(2);
                tracking += 10. * form.entropy * (0.8 + 0.65 * wave);
                displacement =
                    Vec2::new(wave * 15., (index_phase * 1.88 + time * 0.15).cos() * 49.)
                        * disintegrate;
                angle = disintegrate * wave * 0.22;
                mask = 1. - disintegrate * (0.18 + 0.71 * ((index_phase * 5.7).sin() * 0.5 + 0.5));
                pigment = pigment.alpha(1. - disintegrate * (0.22 + wave * 0.28));
                if form.entropy > 0.69 && wave > 0.51 && ch != ' ' {
                    symbol = ['.', 'O', '-', 'I'][index % 4];
                }
            }
            let original = self.glyph(style.face, ch);
            let Glyph { metric, mut uv } = self.glyph(style.face, symbol);
            let size = Vec2::new(metric.width as f32, metric.height as f32) * scale;
            if size.min_element() > 0. {
                let offset = Vec2::new(
                    metric.xmin as f32 * scale,
                    -size.y - metric.ymin as f32 * scale,
                );
                uv[3] = uv[1] + (uv[3] - uv[1]) * mask;
                layer.glyph(
                    Vec2::new(pen, baseline.y) + offset + displacement,
                    size * Vec2::new(1., mask),
                    angle,
                    pigment,
                    uv,
                );
            }
            pen += original.metric.advance_width * scale + tracking;
        }
    }
}
