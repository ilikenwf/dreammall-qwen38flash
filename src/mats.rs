//! Custom materials: one lit+fog world shader, water, sprites, mirror glass.
//! macroquad 0.4.16 shader DSL: GLSL ES 100, built-ins `Model`, `Projection`,
//! attributes `position`/`texcoord`/`color0`, texture sampler `Texture`.

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, CullFace, Equation, UniformDesc};
use macroquad::prelude::*;

pub const VS_WORLD: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying vec2 uv;
varying vec4 vcolor;
varying vec3 vworld;
varying float vdist;

uniform mat4 Model;
uniform mat4 Projection;
uniform vec3 uCamPos;

void main()
{
    vec4 wp = Model * vec4(position, 1.0);
    gl_Position = Projection * wp;
    uv = texcoord;
    vcolor = color0 / 255.0;
    vworld = wp.xyz;
    vdist = distance(uCamPos, wp.xyz);
}
"#;

pub const FS_WORLD: &str = r#"#version 100
precision mediump float;

varying vec2 uv;
varying vec4 vcolor;
varying vec3 vworld;
varying float vdist;

uniform sampler2D Texture;
uniform vec4 uTint;
uniform float uEmissive;
uniform float uUseTex;
uniform vec3 uFogColor;
uniform float uFogDensity;

void main()
{
    vec4 t = mix(vec4(1.0), texture2D(Texture, uv), uUseTex);
    vec4 base = vcolor * uTint * t;
    float ao = clamp(0.62 + 0.38 * (vworld.y / 5.0), 0.45, 1.0);
    vec3 c = mix(base.rgb * ao, base.rgb, uEmissive);
    float f = 1.0 - exp(-uFogDensity * uFogDensity * vdist * vdist);
    c = mix(c, uFogColor, clamp(f, 0.0, 1.0));
    gl_FragColor = vec4(c, base.a);
}
"#;

pub const FS_WATER: &str = r#"#version 100
precision mediump float;

varying vec2 uv;
varying vec4 vcolor;
varying vec3 vworld;
varying float vdist;

uniform vec4 uTint;
uniform vec3 uFogColor;
uniform float uFogDensity;
uniform float uTime;

void main()
{
    vec2 p = vworld.xz;
    float w = sin(p.x * 2.1 + uTime * 1.7) * 0.5 + 0.5;
    float w2 = sin(p.y * 2.7 - uTime * 1.3) * 0.5 + 0.5;
    float r = w * w2;
    vec3 c = mix(uTint.rgb * 0.45, uTint.rgb, r * 0.85 + 0.1);
    float sp = pow(max(0.0, sin(p.x * 9.0 + uTime * 3.0) * sin(p.y * 11.0 - uTime * 2.2)), 12.0);
    c += vec3(sp * 0.5);
    float f = 1.0 - exp(-uFogDensity * uFogDensity * vdist * vdist);
    c = mix(c, uFogColor, clamp(f, 0.0, 1.0));
    gl_FragColor = vec4(c, uTint.a);
}
"#;

pub const VS_FLAT: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying vec2 uv;
varying vec4 vcolor;

uniform mat4 Model;
uniform mat4 Projection;

void main()
{
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
    vcolor = color0 / 255.0;
}
"#;

pub const FS_SPRITE: &str = r#"#version 100
precision mediump float;

varying vec2 uv;
varying vec4 vcolor;

uniform sampler2D Texture;
uniform vec4 uTint;

void main()
{
    vec4 c = texture2D(Texture, uv) * vcolor * uTint;
    if (c.a < 0.02) discard;
    gl_FragColor = c;
}
"#;

pub const FS_FLAT: &str = r#"#version 100
precision mediump float;

varying vec2 uv;
varying vec4 vcolor;

uniform sampler2D Texture;
uniform vec4 uTint;

void main()
{
    gl_FragColor = texture2D(Texture, uv) * vcolor * uTint;
}
"#;

fn alpha_blend() -> BlendState {
    BlendState::new(
        Equation::Add,
        BlendFactor::Value(BlendValue::SourceAlpha),
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    )
}

fn mat(
    vs: &str,
    fs: &str,
    uniforms: Vec<(String, UniformType)>,
    depth_write: bool,
    cull: CullFace,
) -> Material {
    let shader = ShaderSource::Glsl {
        vertex: vs,
        fragment: fs,
    };
    let params = MaterialParams {
        pipeline_params: PipelineParams {
            color_blend: Some(alpha_blend()),
            alpha_blend: Some(alpha_blend()),
            depth_test: Comparison::LessOrEqual,
            depth_write,
            cull_face: cull,
            ..Default::default()
        },
        uniforms: uniforms
            .into_iter()
            .map(|(name, uniform_type)| UniformDesc {
                name,
                uniform_type,
                array_count: 1,
            })
            .collect(),
        textures: vec![],
    };
    load_material(shader, params).expect("shader compile")
}

fn uf(name: &str, t: UniformType) -> (String, UniformType) {
    (name.to_string(), t)
}


fn world_uniforms() -> Vec<(String, UniformType)> {
    vec![
        uf("uTint", UniformType::Float4),
        uf("uEmissive", UniformType::Float1),
        uf("uUseTex", UniformType::Float1),
        uf("uFogColor", UniformType::Float3),
        uf("uFogDensity", UniformType::Float1),
        uf("uCamPos", UniformType::Float3),
    ]
}

pub struct Mats {
    pub world_tex: Material,
    pub world_flat: Material,
    pub emissive: Material,
    pub sign: Material,
    pub glass: Material,
    pub water: Material,
    pub mirror: Material,
    pub sprite: Material,
}

impl Mats {
    pub fn new() -> Mats {
        let m = Mats {
            world_tex: mat(
                VS_WORLD,
                FS_WORLD,
                world_uniforms(),
                true,
                CullFace::Nothing,
            ),
            world_flat: mat(
                VS_WORLD,
                FS_WORLD,
                world_uniforms(),
                true,
                CullFace::Nothing,
            ),
            emissive: mat(
                VS_WORLD,
                FS_WORLD,
                world_uniforms(),
                true,
                CullFace::Nothing,
            ),
            sign: mat(
                VS_WORLD,
                FS_WORLD,
                world_uniforms(),
                true,
                CullFace::Nothing,
            ),
            glass: mat(
                VS_WORLD,
                FS_WORLD,
                world_uniforms(),
                false,
                CullFace::Nothing,
            ),
            water: mat(
                VS_WORLD,
                FS_WATER,
                vec![
                    uf("uTint", UniformType::Float4),
                    uf("uFogColor", UniformType::Float3),
                    uf("uFogDensity", UniformType::Float1),
                    uf("uTime", UniformType::Float1),
                    uf("uCamPos", UniformType::Float3),
                ],
                false,
                CullFace::Nothing,
            ),
            mirror: mat(
                VS_FLAT,
                FS_FLAT,
                vec![uf("uTint", UniformType::Float4)],
                true,
                CullFace::Nothing,
            ),
            sprite: mat(
                VS_FLAT,
                FS_SPRITE,
                vec![uf("uTint", UniformType::Float4)],
                false,
                CullFace::Nothing,
            ),
        };
        // initial uniform values
        m.world_tex.set_uniform("uUseTex", 1.0f32);
        m.world_flat.set_uniform("uUseTex", 0.0f32);
        m.emissive.set_uniform("uEmissive", 1.0f32);
        m.emissive.set_uniform("uUseTex", 0.0f32);
        m.sign.set_uniform("uEmissive", 1.0f32);
        m.sign.set_uniform("uUseTex", 1.0f32);
        m.glass.set_uniform("uUseTex", 0.0f32);
        m.glass.set_uniform("uEmissive", 0.55f32);
        for x in [&m.world_tex, &m.world_flat, &m.emissive, &m.sign, &m.glass, &m.water] {
            x.set_uniform("uTint", vec4(1.0, 1.0, 1.0, 1.0));
        }
        m.mirror.set_uniform("uTint", vec4(1.0, 1.0, 1.0, 1.0));
        m.sprite.set_uniform("uTint", vec4(1.0, 1.0, 1.0, 1.0));
        m
    }

    /// Per-frame fog/camera uniforms for the mall passes.
    pub fn set_fog(&self, fog: [f32; 3], density: f32, tint: [f32; 3], cam: Vec3) {
        for m in [&self.world_tex, &self.world_flat, &self.emissive, &self.sign, &self.glass] {
            m.set_uniform("uFogColor", vec3(fog[0], fog[1], fog[2]));
            m.set_uniform("uFogDensity", density);
            m.set_uniform("uTint", vec4(tint[0], tint[1], tint[2], 1.0));
            m.set_uniform("uCamPos", cam);
        }
        self.water.set_uniform("uFogColor", vec3(fog[0], fog[1], fog[2]));
        self.water.set_uniform("uFogDensity", density);
        self.water.set_uniform("uCamPos", cam);
    }
}
