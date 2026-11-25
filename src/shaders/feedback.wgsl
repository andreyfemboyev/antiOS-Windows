struct Signal {
    time: f32,
    fracture: f32,
    persistence: f32,
    srgb: f32,
}
@group(0) @binding(0) var present: texture_2d<f32>;
@group(0) @binding(1) var past: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var<uniform> signal: Signal;

struct Fragment {
    @builtin(position) pixel: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex fn vertex(@builtin(vertex_index) index: u32) -> Fragment {
    var corners = array<vec2<f32>, 3>(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.));
    let point = corners[index];
    var out: Fragment;
    out.pixel = vec4(point, 0., 1.);
    out.uv = point * vec2(0.5, -0.5) + vec2(0.5);
    return out;
}

@fragment fn history(in: Fragment) -> @location(0) vec4<f32> {
    let p = in.uv;
    let position = p - vec2(0.54, 0.5);
    let bend = vec2(sin(p.y * 19. + signal.time * 0.47), cos(p.x * 15. - signal.time * 0.31));
    let warped = p + bend * signal.fracture * 0.002;
    let current = textureSample(present, linear_sampler, warped).rgb;
    let drift = p - position * (0.001 + signal.fracture * 0.012) + bend * signal.fracture * 0.003;
    let previous = textureSample(past, linear_sampler, drift).rgb;
    let luminance = dot(current, vec3(0.27, 0.61, 0.12));
    let imprint = max(previous - current, vec3(0.));
    let retention = signal.persistence * (1. - smoothstep(0.04, 0.19, luminance));
    return vec4(current + imprint * retention, 1.);
}

@fragment fn display(in: Fragment) -> @location(0) vec4<f32> {
    let color = textureSample(present, linear_sampler, in.uv).rgb;
    let linear = select(color / 12.92, pow((color + 0.055) / 1.055, vec3(2.4)), color > vec3(0.04045));
    return vec4(mix(color, linear, signal.srgb), 1.);
}
