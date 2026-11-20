use crate::camera::project;
use anyhow::{Context, Result};
use glb_viewer::scene::{Bounds, Scene};
use macroquad::{prelude::*, window::get_internal_gl};
use miniquad as mq;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Shaded,
    Wireframe,
    Overlay,
}
pub struct Batch {
    primitive: usize,
    source: Vec<usize>,
    vertices: Vec<Vertex>,
    vertex_buffer: mq::BufferId,
    triangles: mq::BufferId,
    edges: mq::BufferId,
    triangle_count: i32,
    edge_count: i32,
    center: Vec3,
}
pub struct GpuScene {
    batches: Vec<Batch>,
    textures: Vec<Texture2D>,
    texture_ids: Vec<mq::TextureId>,
}
impl GpuScene {
    pub fn new(scene: &Scene) -> Self {
        let textures: Vec<_> = scene
            .textures
            .iter()
            .map(|info| {
                let image = &scene.images[info.image];
                let t = Texture2D::from_rgba8(image.width, image.height, &image.rgba);
                t.set_filter(if info.nearest {
                    FilterMode::Nearest
                } else {
                    FilterMode::Linear
                });
                let wrap = |w| match w {
                    gltf::texture::WrappingMode::ClampToEdge => mq::TextureWrap::Clamp,
                    gltf::texture::WrappingMode::MirroredRepeat => mq::TextureWrap::Mirror,
                    _ => mq::TextureWrap::Repeat,
                };
                unsafe {
                    get_internal_gl().quad_context.texture_set_wrap(
                        t.raw_miniquad_id(),
                        wrap(info.wrap_s),
                        wrap(info.wrap_t),
                    );
                }
                t
            })
            .collect();
        let texture_ids = textures.iter().map(Texture2D::raw_miniquad_id).collect();
        // These context borrows are confined to the application's render thread.
        let ctx = unsafe { get_internal_gl().quad_context };
        let mut batches = Vec::new();
        for (primitive, p) in scene.primitives.iter().enumerate() {
            // Portable u16 indices: remap independent chunks on triangle boundaries.
            for chunk in p.indices.chunks(60_000) {
                let mut map = HashMap::new();
                let mut source = Vec::new();
                let indices: Vec<u16> = chunk
                    .iter()
                    .map(|&index| {
                        *map.entry(index).or_insert_with(|| {
                            let local = source.len() as u16;
                            source.push(index as usize);
                            local
                        })
                    })
                    .collect();
                let vertices: Vec<_> = source
                    .iter()
                    .map(|&i| Vertex {
                        position: p.posed_positions[i],
                        uv: p.uvs[i],
                        color: p.colors[i],
                        normal: p.posed_normals[i].extend(0.0),
                    })
                    .collect();
                let mut seen = HashSet::new();
                let mut edges = Vec::new();
                for t in indices.chunks_exact(3) {
                    for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                        if seen.insert((a.min(b), a.max(b))) {
                            edges.extend_from_slice(&[a, b]);
                        }
                    }
                }
                let center = vertices.iter().fold(Vec3::ZERO, |sum, v| sum + v.position)
                    / vertices.len().max(1) as f32;
                let vertex_buffer = ctx.new_buffer(
                    mq::BufferType::VertexBuffer,
                    mq::BufferUsage::Dynamic,
                    mq::BufferSource::slice(&vertices),
                );
                let triangles = ctx.new_buffer(
                    mq::BufferType::IndexBuffer,
                    mq::BufferUsage::Immutable,
                    mq::BufferSource::slice(&indices),
                );
                let edge_buffer = ctx.new_buffer(
                    mq::BufferType::IndexBuffer,
                    mq::BufferUsage::Immutable,
                    mq::BufferSource::slice(&edges),
                );
                batches.push(Batch {
                    primitive,
                    source,
                    vertices,
                    vertex_buffer,
                    triangles,
                    edges: edge_buffer,
                    triangle_count: indices.len() as i32,
                    edge_count: edges.len() as i32,
                    center,
                });
            }
        }
        Self {
            batches,
            textures,
            texture_ids,
        }
    }
    pub fn update(&mut self, scene: &Scene) {
        let ctx = unsafe { get_internal_gl().quad_context };
        for batch in &mut self.batches {
            let p = &scene.primitives[batch.primitive];
            let mut center = Vec3::ZERO;
            for (v, &index) in batch.vertices.iter_mut().zip(&batch.source) {
                v.position = p.posed_positions[index];
                v.normal = p.posed_normals[index].extend(0.0);
                center += v.position;
            }
            batch.center = center / batch.vertices.len().max(1) as f32;
            ctx.buffer_update(
                batch.vertex_buffer,
                mq::BufferSource::slice(&batch.vertices),
            );
        }
    }
}
impl Drop for GpuScene {
    fn drop(&mut self) {
        let ctx = unsafe { get_internal_gl().quad_context };
        for b in &self.batches {
            ctx.delete_buffer(b.vertex_buffer);
            ctx.delete_buffer(b.triangles);
            ctx.delete_buffer(b.edges);
        }
        // Texture2D's reference-counted ownership releases the image objects.
        self.textures.clear();
    }
}
#[repr(C)]
struct Uniforms {
    model: Mat4,
    projection: Mat4,
    time: Vec4,
    eye_exposure: Vec4,
    base_color: Vec4,
    surface: Vec4,
    emission: Vec4,
    options: Vec4,
}
#[repr(C)]
struct WireUniforms {
    model: Mat4,
    projection: Mat4,
    time: Vec4,
    tint: Vec4,
}
pub struct Renderer {
    surfaces: Vec<mq::Pipeline>,
    wire: mq::Pipeline,
    white: Texture2D,
}
impl Renderer {
    pub fn new() -> Result<Self> {
        let ctx = unsafe { get_internal_gl().quad_context };
        let backend = ctx.info().backend;
        let fragment = concat!(
            "#version 120\n",
            include_str!("shaders/lighting.inc"),
            include_str!("shaders/surface.frag")
        );
        let metal = concat!(
            "#include <metal_stdlib>\nusing namespace metal;\ntypedef float3 vec3;\n",
            include_str!("shaders/lighting.inc"),
            include_str!("shaders/surface.metal")
        );
        let mut uniforms = vec![
            mq::UniformDesc::new("Model", mq::UniformType::Mat4),
            mq::UniformDesc::new("Projection", mq::UniformType::Mat4),
            mq::UniformDesc::new("_Time", mq::UniformType::Float4),
        ];
        uniforms.extend(
            ["EyeExposure", "BaseColor", "Surface", "Emission", "Options"]
                .into_iter()
                .map(|n| mq::UniformDesc::new(n, mq::UniformType::Float4)),
        );
        let shader = ctx
            .new_shader(
                match backend {
                    mq::Backend::OpenGl => mq::ShaderSource::Glsl {
                        vertex: include_str!("shaders/surface.vert"),
                        fragment,
                    },
                    mq::Backend::Metal => mq::ShaderSource::Msl { program: metal },
                },
                mq::ShaderMeta {
                    uniforms: mq::UniformBlockLayout { uniforms },
                    images: [
                        "Texture",
                        "MetallicRoughness",
                        "NormalMap",
                        "Occlusion",
                        "EmissiveMap",
                    ]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                },
            )
            .context("Surface shader")?;
        let attributes = [
            mq::VertexAttribute::new("position", mq::VertexFormat::Float3),
            mq::VertexAttribute::new("texcoord", mq::VertexFormat::Float2),
            mq::VertexAttribute::new("color0", mq::VertexFormat::Byte4),
            mq::VertexAttribute::new("normal", mq::VertexFormat::Float4),
        ];
        let mut surfaces = Vec::new();
        for blend in [false, true] {
            for double_sided in [false, true] {
                surfaces.push(ctx.new_pipeline(
                    &[mq::BufferLayout::default()],
                    &attributes,
                    shader,
                    mq::PipelineParams {
                        depth_test: mq::Comparison::LessOrEqual,
                        depth_write: !blend,
                        cull_face: if double_sided {
                            mq::CullFace::Nothing
                        } else {
                            mq::CullFace::Back
                        },
                        color_blend: blend.then(|| {
                            mq::BlendState::new(
                                mq::Equation::Add,
                                mq::BlendFactor::Value(mq::BlendValue::SourceAlpha),
                                mq::BlendFactor::OneMinusValue(mq::BlendValue::SourceAlpha),
                            )
                        }),
                        ..Default::default()
                    },
                ));
            }
        }
        let shader = ctx
            .new_shader(
                match backend {
                    mq::Backend::OpenGl => mq::ShaderSource::Glsl {
                        vertex: WIRE_VERTEX,
                        fragment: WIRE_FRAGMENT,
                    },
                    mq::Backend::Metal => mq::ShaderSource::Msl {
                        program: WIRE_METAL,
                    },
                },
                mq::ShaderMeta {
                    uniforms: mq::UniformBlockLayout {
                        uniforms: vec![
                            mq::UniformDesc::new("Model", mq::UniformType::Mat4),
                            mq::UniformDesc::new("Projection", mq::UniformType::Mat4),
                            mq::UniformDesc::new("_Time", mq::UniformType::Float4),
                            mq::UniformDesc::new("Tint", mq::UniformType::Float4),
                        ],
                    },
                    images: vec![],
                },
            )
            .context("Wireframe shader")?;
        let wire = ctx.new_pipeline(
            &[mq::BufferLayout::default()],
            &attributes,
            shader,
            mq::PipelineParams {
                primitive_type: mq::PrimitiveType::Lines,
                depth_test: mq::Comparison::LessOrEqual,
                depth_write: false,
                ..Default::default()
            },
        );
        Ok(Self {
            surfaces,
            wire,
            white: Texture2D::from_rgba8(1, 1, &[255; 4]),
        })
    }
    pub fn draw(
        &self,
        scene: &Scene,
        gpu: &GpuScene,
        camera: &Camera3D,
        mode: ViewMode,
        exposure: f32,
    ) {
        use macroquad::camera::Camera;
        let white = self.white.raw_miniquad_id();
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        ctx.begin_default_pass(mq::PassAction::Nothing);
        if let Some((x, y, w, h)) = camera.viewport {
            ctx.apply_viewport(x, y, w, h);
            ctx.apply_scissor_rect(x, y, w, h);
        }
        let projection = camera.matrix();
        if mode != ViewMode::Wireframe {
            let mut order: Vec<_> = (0..gpu.batches.len()).collect();
            order.sort_by(|a, b| {
                let ba = &gpu.batches[*a];
                let bb = &gpu.batches[*b];
                let ma = &scene.materials[scene.primitives[ba.primitive].material];
                let mb = &scene.materials[scene.primitives[bb.primitive].material];
                ma.blend.cmp(&mb.blend).then_with(|| {
                    if ma.blend {
                        bb.center
                            .distance_squared(camera.position)
                            .total_cmp(&ba.center.distance_squared(camera.position))
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
            });
            for index in order {
                let batch = &gpu.batches[index];
                let p = &scene.primitives[batch.primitive];
                let s = &scene.materials[p.material];
                let double_sided = s.double_sided || scene.world[p.node].determinant() < 0.0;
                ctx.apply_pipeline(&self.surfaces[s.blend as usize * 2 + double_sided as usize]);
                let images = [
                    s.base_texture,
                    s.mr_texture,
                    s.normal_texture,
                    s.ao_texture,
                    s.emissive_texture,
                ]
                .into_iter()
                .map(|i| i.map_or(white, |i| gpu.texture_ids[i]))
                .collect();
                ctx.apply_bindings(&mq::Bindings {
                    vertex_buffers: vec![batch.vertex_buffer],
                    index_buffer: batch.triangles,
                    images,
                });
                ctx.apply_uniforms(mq::UniformsSource::table(&Uniforms {
                    model: Mat4::IDENTITY,
                    projection,
                    time: Vec4::ZERO,
                    eye_exposure: camera.position.extend(exposure),
                    base_color: s.base_color,
                    surface: vec4(
                        s.metallic,
                        s.roughness,
                        s.alpha_cutoff,
                        s.unlit as u8 as f32,
                    ),
                    emission: s.emissive.extend(s.normal_scale),
                    options: vec4(
                        s.normal_texture.is_some() as u8 as f32,
                        s.ao_strength,
                        (!s.blend) as u8 as f32,
                        0.0,
                    ),
                }));
                ctx.draw(0, batch.triangle_count, 1);
            }
        }
        if mode != ViewMode::Shaded {
            ctx.apply_pipeline(&self.wire);
            let tint = if mode == ViewMode::Wireframe {
                vec4(0.39, 0.89, 0.79, 1.0)
            } else {
                vec4(0.06, 0.22, 0.23, 1.0)
            };
            ctx.apply_uniforms(mq::UniformsSource::table(&WireUniforms {
                model: Mat4::IDENTITY,
                projection,
                time: Vec4::ZERO,
                tint,
            }));
            for batch in &gpu.batches {
                ctx.apply_bindings(&mq::Bindings {
                    vertex_buffers: vec![batch.vertex_buffer],
                    index_buffer: batch.edges,
                    images: vec![],
                });
                ctx.draw(0, batch.edge_count, 1);
            }
        }
        ctx.end_render_pass();
    }
}
pub fn grid(bounds: Bounds) {
    let radius = bounds.radius();
    let step = 10.0_f32.powf((radius / 3.0).log10().floor());
    let step = if radius / step > 12.0 {
        step * 5.0
    } else if radius / step > 6.0 {
        step * 2.0
    } else {
        step
    };
    let center = bounds.center();
    let y = bounds.min.y - radius * 0.008;
    let cx = (center.x / step).round() * step;
    let cz = (center.z / step).round() * step;
    let n = 14;
    for i in -n..=n {
        let fade = 1.0 - (i as f32 / (n + 1) as f32).abs();
        let c = Color::new(0.22, 0.28, 0.33, 0.13 + 0.30 * fade);
        draw_line_3d(
            vec3(cx + i as f32 * step, y, cz - n as f32 * step),
            vec3(cx + i as f32 * step, y, cz + n as f32 * step),
            c,
        );
        draw_line_3d(
            vec3(cx - n as f32 * step, y, cz + i as f32 * step),
            vec3(cx + n as f32 * step, y, cz + i as f32 * step),
            c,
        );
    }
}
pub fn skeleton(scene: &Scene, matrix: Mat4, viewport: Rect) {
    let position = |i: usize| {
        project(
            scene.world[i].transform_point3(Vec3::ZERO),
            matrix,
            viewport,
        )
        .filter(|p| viewport.contains(*p))
    };
    for &(a, b) in &scene.bones {
        if let (Some(a), Some(b)) = (position(a), position(b)) {
            draw_line(a.x, a.y, b.x, b.y, 5.0, Color::new(0.02, 0.09, 0.10, 0.9));
            draw_line(a.x, a.y, b.x, b.y, 2.2, Color::from_hex(0x6cf0ce));
        }
    }
    for &j in &scene.joint_nodes {
        if let Some(p) = position(j) {
            draw_circle(p.x, p.y, 4.0, Color::from_hex(0x0b2528));
            draw_circle(p.x, p.y, 2.4, Color::from_hex(0xb8ffe8));
        }
    }
}
const WIRE_VERTEX: &str = "#version 120\nattribute vec3 position; uniform mat4 Model; uniform mat4 Projection; void main(){ gl_Position=Projection*Model*vec4(position,1.0); gl_Position.z-=0.00008*gl_Position.w; }";
const WIRE_FRAGMENT: &str = "#version 120\nuniform vec4 Tint; void main(){ gl_FragColor=Tint; }";
const WIRE_METAL: &str = r#"
#include <metal_stdlib>
using namespace metal;
struct Vertex { float3 position [[attribute(0)]]; };
struct Uniforms { float4x4 Model; float4x4 Projection; float4 _Time; float4 Tint; };
struct Varying { float4 position [[position]]; };
vertex Varying vertexShader(Vertex v [[stage_in]], constant Uniforms& u [[buffer(0)]]) { Varying o; o.position=u.Projection*u.Model*float4(v.position,1.0); o.position.z-=0.00008*o.position.w; return o; }
fragment float4 fragmentShader(constant Uniforms& u [[buffer(0)]]) { return u.Tint; }
"#;
