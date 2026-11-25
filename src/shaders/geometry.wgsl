struct Canvas {
    size: vec2<f32>,
    zoom: f32,
    _padding: f32,
}
@group(0) @binding(0) var<uniform> canvas: Canvas;
@group(0) @binding(1) var glyphs: texture_2d<f32>;
@group(0) @binding(2) var glyph_sampler: sampler;

struct Instance {
    @location(0) shape: vec4<f32>,
    @location(1) style: vec4<f32>,
    @location(2) pigment: vec4<f32>,
    @location(3) bounds: vec4<f32>,
}
struct Fragment {
    @builtin(position) pixel: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) halfsize: vec2<f32>,
    @location(2) style: vec3<f32>,
    @location(3) pigment: vec4<f32>,
    @location(4) uv: vec2<f32>,
}

@vertex fn vertex(in: Instance, @builtin(vertex_index) index: u32) -> Fragment {
    var corners = array<vec2<f32>, 6>(
        vec2(-1., -1.), vec2(1., -1.), vec2(1., 1.),
        vec2(-1., -1.), vec2(1., 1.), vec2(-1., 1.)
    );
    let local = corners[index];
    let is_glyph = in.style.y >= 4.5;
    let pad = select(1.55 / canvas.zoom, 0., is_glyph);
    let point = local * (in.shape.zw + vec2(pad));
    let s = sin(in.style.x);
    let c = cos(in.style.x);
    let rotated = vec2(c * point.x - s * point.y, s * point.x + c * point.y);
    let screen = (in.shape.xy + rotated - vec2(720., 450.)) * canvas.zoom / canvas.size;
    var out: Fragment;
    out.pixel = vec4(screen * vec2(2., -2.), 0., 1.);
    out.point = point;
    out.halfsize = in.shape.zw;
    out.style = in.style.yzw;
    out.pigment = in.pigment;
    out.uv = mix(in.bounds.xy, in.bounds.zw, local * 0.5 + 0.5);
    return out;
}

@fragment fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    let kind = in.style.x;
    if kind >= 4.5 {
        return vec4(in.pigment.rgb, in.pigment.a * textureSample(glyphs, glyph_sampler, in.uv).r);
    }
    let box = abs(in.point) - in.halfsize;
    var d = length(max(box, vec2(0.))) + min(max(box.x, box.y), 0.);
    if kind >= 2.5 && kind < 3.5 {
        let radius = in.halfsize.y;
        d = length(vec2(max(abs(in.point.x) - (in.halfsize.x - radius), 0.), in.point.y)) - radius;
    }
    if kind >= 1.5 && kind < 2.5 {
        let p = in.point / in.halfsize;
        d = (length(p) - 1.) * min(in.halfsize.x, in.halfsize.y);
    }
    if (kind >= 0.5 && kind < 1.5) || kind >= 3.5 {
        if kind >= 3.5 {
            d = (length(in.point / in.halfsize) - 1.) * min(in.halfsize.x, in.halfsize.y);
        }
        d = abs(d + in.style.y * 0.5) - in.style.y * 0.5;
    }
    let aa = max(fwidth(d) * 0.55, 0.35 / canvas.zoom);
    let alpha = in.pigment.a * (1. - smoothstep(-aa, aa, d));
    return vec4(in.pigment.rgb, alpha);
}
