// pool render-core: 単一パイプライン・モード切替式コンポジットシェーダ。
// mode: 0=手続き図形を over 合成 / 1=テクスチャ矩形を over 合成 /
//       2=明度 / 3=ブラー(方向は blur_dir) / 4=extra 全面を over 合成

struct U {
    resolution: vec2<f32>,
    mode: u32,
    shape: u32,
    color: vec4<f32>,
    opacity: f32,
    _pad0: f32,
    center: vec2<f32>,
    size: vec2<f32>,
    brightness: f32,
    blur_radius: f32,
    blur_dir: vec2<f32>,
    _pad1: vec2<f32>,
    uv_rect: vec4<f32>,
    rotation: f32,
    _pad2: f32,
    anchor: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src_tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var extra_tex: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[i], 0.0, 1.0);
}

fn over(fg: vec4<f32>, bg: vec4<f32>) -> vec4<f32> {
    // premultiplied over
    return fg + bg * (1.0 - fg.a);
}

fn rot_pos(pos: vec2<f32>) -> vec2<f32> {
    let rel = pos - u.center - u.anchor;
    let c = cos(u.rotation);
    let s = sin(u.rotation);
    return vec2<f32>(c * rel.x + s * rel.y, -s * rel.x + c * rel.y) + u.anchor;
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = pos.xy / u.resolution;
    let bg = textureSample(src_tex, samp, uv);
    switch u.mode {
        case 0u: {
            let p = rot_pos(pos.xy) / (u.size * 0.5 + vec2<f32>(1e-4));
            var a: f32 = 0.0;
            if u.shape == 0u {
                if abs(p.x) <= 1.0 && abs(p.y) <= 1.0 {
                    a = 1.0;
                }
            } else {
                a = 1.0 - smoothstep(0.96, 1.0, length(p));
            }
            let at = u.color.a * a * u.opacity;
            return over(vec4<f32>(u.color.rgb * at, at), bg);
        }
        case 1u: {
            let local = rot_pos(pos.xy) / (u.size + vec2<f32>(1e-4)) + vec2<f32>(0.5);
            if local.x < 0.0 || local.x > 1.0 || local.y < 0.0 || local.y > 1.0 {
                return bg;
            }
            let t = textureSample(extra_tex, samp, u.uv_rect.xy + local * u.uv_rect.zw);
            let at = t.a * u.color.a * u.opacity;
            return over(vec4<f32>(u.color.rgb * at, at), bg);
        }
        case 2u: {
            return vec4<f32>(clamp(bg.rgb + vec3<f32>(u.brightness), vec3<f32>(0.0), vec3<f32>(1.0)), bg.a);
        }
        case 3u: {
            let stepv = u.blur_dir * u.blur_radius / (4.0 * u.resolution);
            var acc = vec4<f32>(0.0);
            var wsum: f32 = 0.0;
            for (var i: i32 = -4; i <= 4; i = i + 1) {
                let w = exp(-f32(i * i) / 8.0);
                acc = acc + textureSample(src_tex, samp, uv + stepv * f32(i)) * w;
                wsum = wsum + w;
            }
            return acc / wsum;
        }
        case 5u: {
            // 図形を直接出力（ブレンドパイプラインで合成）
            let p = rot_pos(pos.xy) / (u.size * 0.5 + vec2<f32>(1e-4));
            var a: f32 = 0.0;
            if u.shape == 0u {
                if abs(p.x) <= 1.0 && abs(p.y) <= 1.0 {
                    a = 1.0;
                }
            } else {
                a = 1.0 - smoothstep(0.96, 1.0, length(p));
            }
            let at = u.color.a * a * u.opacity;
            return vec4<f32>(u.color.rgb * at, at);
        }
        case 6u: {
            // テクスチャ矩形を直接出力（ブレンドパイプラインで合成）
            let local = rot_pos(pos.xy) / (u.size + vec2<f32>(1e-4)) + vec2<f32>(0.5);
            if local.x < 0.0 || local.x > 1.0 || local.y < 0.0 || local.y > 1.0 {
                return vec4<f32>(0.0);
            }
            let t = textureSample(extra_tex, samp, u.uv_rect.xy + local * u.uv_rect.zw);
            let at = t.a * u.color.a * u.opacity;
            return vec4<f32>(u.color.rgb * at, at);
        }
        default: {
            // 素材をそのまま出力。合成はブレンドパイプラインに任せる。
            return textureSample(extra_tex, samp, uv);
        }
    }
}
