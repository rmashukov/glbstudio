use crate::animation::{Channel, Clip};
use anyhow::{Context, Result, bail, ensure};
use gltf::{
    animation::{Interpolation, Property, util::ReadOutputs},
    mesh::Mode,
};
use macroquad::prelude::*;
use std::{
    collections::{HashSet, VecDeque},
    path::Path,
};

pub const DEMO: &[u8] = include_bytes!("../assets/orbit-bot.glb");

#[derive(Clone, Copy)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl Transform {
    fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}
pub struct Node {
    pub parent: Option<usize>,
    pub base: Transform,
    pub matrix: Mat4,
    pub weights: Vec<f32>,
}
pub struct Skin {
    pub joints: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
}
pub struct Morph {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}
pub struct Primitive {
    pub node: usize,
    pub skin: Option<usize>,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<[u8; 4]>,
    pub joints: Vec<[u16; 8]>,
    pub weights: Vec<[f32; 8]>,
    pub morphs: Vec<Morph>,
    pub indices: Vec<u32>,
    pub material: usize,
    pub posed_positions: Vec<Vec3>,
    pub posed_normals: Vec<Vec3>,
}
pub struct ImageData {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}
pub struct TextureInfo {
    pub image: usize,
    pub wrap_s: gltf::texture::WrappingMode,
    pub wrap_t: gltf::texture::WrappingMode,
    pub nearest: bool,
}
pub struct Surface {
    pub base_color: Vec4,
    pub base_texture: Option<usize>,
    pub mr_texture: Option<usize>,
    pub normal_texture: Option<usize>,
    pub ao_texture: Option<usize>,
    pub emissive_texture: Option<usize>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
    pub normal_scale: f32,
    pub ao_strength: f32,
    pub alpha_cutoff: f32,
    pub blend: bool,
    pub double_sided: bool,
    pub unlit: bool,
    pub texcoord: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}
impl Bounds {
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    pub fn radius(self) -> f32 {
        ((self.max - self.min).length() * 0.5).max(0.001)
    }
}
pub struct Scene {
    pub name: String,
    pub nodes: Vec<Node>,
    pub order: Vec<usize>,
    pub world: Vec<Mat4>,
    pub skins: Vec<Skin>,
    pub primitives: Vec<Primitive>,
    pub images: Vec<ImageData>,
    pub textures: Vec<TextureInfo>,
    pub materials: Vec<Surface>,
    pub clips: Vec<Clip>,
    pub bones: Vec<(usize, usize)>,
    pub joint_nodes: Vec<usize>,
    pub bounds: Bounds,
    pub warnings: Vec<String>,
}

impl Scene {
    pub fn from_path(path: &Path) -> Result<Self> {
        let imported =
            gltf::import(path).with_context(|| format!("Cannot open {}", path.display()))?;
        Self::from_import(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            imported,
        )
    }
    pub fn demo() -> Result<Self> {
        Self::from_import("Orbit bot".into(), gltf::import_slice(DEMO)?)
    }
    pub fn from_import(
        name: String,
        (doc, buffers, images): (
            gltf::Document,
            Vec<gltf::buffer::Data>,
            Vec<gltf::image::Data>,
        ),
    ) -> Result<Self> {
        let mut warnings = Vec::new();
        for extension in doc.extensions_required() {
            ensure!(
                extension == "KHR_materials_unlit",
                "Required extension {extension} is not supported. Export standard glTF 2.0 (uncompressed PNG/JPEG textures)."
            );
        }
        for extension in doc.extensions_used() {
            if extension != "KHR_materials_unlit" {
                warnings.push(format!("Optional extension ignored: {extension}"));
            }
        }
        let mut nodes: Vec<_> = doc
            .nodes()
            .map(|n| {
                let (t, r, s) = n.transform().decomposed();
                Node {
                    parent: None,
                    base: Transform {
                        translation: Vec3::from(t),
                        rotation: Quat::from_array(r),
                        scale: Vec3::from(s),
                    },
                    matrix: Mat4::from_cols_array_2d(&n.transform().matrix()),
                    weights: n
                        .weights()
                        .or_else(|| n.mesh().and_then(|m| m.weights()))
                        .unwrap_or(&[])
                        .to_vec(),
                }
            })
            .collect();
        let mut children = vec![Vec::new(); nodes.len()];
        for n in doc.nodes() {
            for child in n.children() {
                ensure!(
                    nodes[child.index()].parent.is_none(),
                    "Node has multiple parents"
                );
                nodes[child.index()].parent = Some(n.index());
                children[n.index()].push(child.index());
            }
        }
        let mut queue: VecDeque<_> = nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(i, _)| i)
            .collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            queue.extend(&children[n]);
        }
        ensure!(order.len() == nodes.len(), "Cyclic node hierarchy");
        let roots: Vec<_> = doc
            .default_scene()
            .or_else(|| doc.scenes().next())
            .map(|s| s.nodes().map(|n| n.index()).collect())
            .unwrap_or_else(|| {
                nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.parent.is_none())
                    .map(|(i, _)| i)
                    .collect()
            });
        let mut active = HashSet::new();
        let mut stack = roots;
        while let Some(n) = stack.pop() {
            if active.insert(n) {
                stack.extend(&children[n]);
            }
        }
        let skins: Vec<_> = doc
            .skins()
            .map(|s| {
                let joints: Vec<_> = s.joints().map(|j| j.index()).collect();
                let inverse_bind = s
                    .reader(|b| Some(&buffers[b.index()]))
                    .read_inverse_bind_matrices()
                    .map(|r| r.map(|m| Mat4::from_cols_array_2d(&m)).collect())
                    .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);
                Skin {
                    joints,
                    inverse_bind,
                }
            })
            .collect();
        for s in &skins {
            ensure!(
                s.joints.len() == s.inverse_bind.len(),
                "Invalid inverse bind matrix count"
            );
        }
        let textures = doc
            .textures()
            .map(|t| TextureInfo {
                image: t.source().index(),
                wrap_s: t.sampler().wrap_s(),
                wrap_t: t.sampler().wrap_t(),
                nearest: t.sampler().mag_filter() == Some(gltf::texture::MagFilter::Nearest),
            })
            .collect();
        let mut materials: Vec<_> = doc
            .materials()
            .map(|m| read_material(m, &mut warnings))
            .collect();
        let fallback_material = materials.len();
        materials.push(Surface {
            base_color: Vec4::ONE,
            base_texture: None,
            mr_texture: None,
            normal_texture: None,
            ao_texture: None,
            emissive_texture: None,
            metallic: 1.0,
            roughness: 1.0,
            emissive: Vec3::ZERO,
            normal_scale: 1.0,
            ao_strength: 1.0,
            alpha_cutoff: -1.0,
            blend: false,
            double_sided: false,
            unlit: false,
            texcoord: 0,
        });
        let mut primitives = Vec::new();
        for node in doc.nodes().filter(|n| active.contains(&n.index())) {
            let Some(mesh) = node.mesh() else { continue };
            for p in mesh.primitives() {
                if !matches!(
                    p.mode(),
                    Mode::Triangles | Mode::TriangleStrip | Mode::TriangleFan
                ) {
                    warnings.push("Point/line primitive skipped".into());
                    continue;
                }
                let reader = p.reader(|b| Some(&buffers[b.index()]));
                let positions: Vec<_> = reader
                    .read_positions()
                    .context("Mesh has no POSITION attribute")?
                    .map(Vec3::from)
                    .collect();
                ensure!(
                    positions.iter().all(|p| p.is_finite()),
                    "Non-finite vertex positions"
                );
                let count = positions.len();
                let raw_indices: Vec<u32> = reader
                    .read_indices()
                    .map(|r| r.into_u32().collect())
                    .unwrap_or_else(|| (0..count as u32).collect());
                ensure!(
                    raw_indices.iter().all(|i| (*i as usize) < count),
                    "Mesh index out of bounds"
                );
                let indices = triangulate(&raw_indices, p.mode())?;
                let normals = reader
                    .read_normals()
                    .map(|r| r.map(Vec3::from).collect())
                    .unwrap_or_else(|| generate_normals(&positions, &indices));
                let material = p.material().index().unwrap_or(fallback_material);
                let uvs = reader
                    .read_tex_coords(materials[material].texcoord)
                    .map(|r| r.into_f32().map(Vec2::from).collect())
                    .unwrap_or_else(|| vec![Vec2::ZERO; count]);
                let colors = reader
                    .read_colors(0)
                    .map(|r| {
                        r.into_rgba_f32()
                            .map(|c| c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
                            .collect()
                    })
                    .unwrap_or_else(|| vec![[255; 4]; count]);
                let mut joints = vec![[0; 8]; count];
                let mut weights = vec![[0.0; 8]; count];
                for set in 0..2 {
                    if let (Some(js), Some(ws)) =
                        (reader.read_joints(set), reader.read_weights(set))
                    {
                        for (i, (j, w)) in js.into_u16().zip(ws.into_f32()).enumerate() {
                            ensure!(i < count, "Skin attribute count mismatch");
                            joints[i][set as usize * 4..set as usize * 4 + 4].copy_from_slice(&j);
                            weights[i][set as usize * 4..set as usize * 4 + 4].copy_from_slice(&w);
                        }
                    }
                }
                if let Some(s) = node.skin() {
                    ensure!(
                        reader.read_joints(0).is_some() && reader.read_weights(0).is_some(),
                        "Skinned mesh lacks JOINTS_0/WEIGHTS_0"
                    );
                    for (js, ws) in joints.iter().zip(weights.iter_mut()) {
                        let sum: f32 = ws.iter().sum();
                        ensure!(
                            sum.is_finite() && sum > 0.0 && ws.iter().all(|w| *w >= 0.0),
                            "Invalid skin weights"
                        );
                        for (j, w) in js.iter().zip(ws.iter_mut()) {
                            ensure!(
                                *w == 0.0 || (*j as usize) < skins[s.index()].joints.len(),
                                "Joint index out of bounds"
                            );
                            *w /= sum;
                        }
                    }
                }
                let morphs: Vec<_> = reader
                    .read_morph_targets()
                    .map(|(p, n, _)| Morph {
                        positions: p
                            .map(|r| r.map(Vec3::from).collect())
                            .unwrap_or_else(|| vec![Vec3::ZERO; count]),
                        normals: n
                            .map(|r| r.map(Vec3::from).collect())
                            .unwrap_or_else(|| vec![Vec3::ZERO; count]),
                    })
                    .collect();
                let normals: Vec<Vec3> = normals;
                let uvs: Vec<Vec2> = uvs;
                let colors: Vec<[u8; 4]> = colors;
                ensure!(
                    normals.len() == count && uvs.len() == count && colors.len() == count,
                    "Vertex attribute count mismatch"
                );
                ensure!(
                    morphs
                        .iter()
                        .all(|m| m.positions.len() == count && m.normals.len() == count),
                    "Morph attribute count mismatch"
                );
                if nodes[node.index()].weights.is_empty() {
                    nodes[node.index()].weights = vec![0.0; morphs.len()];
                }
                primitives.push(Primitive {
                    node: node.index(),
                    skin: node.skin().map(|s| s.index()),
                    posed_positions: positions.clone(),
                    posed_normals: normals.clone(),
                    positions,
                    normals,
                    uvs,
                    colors,
                    joints,
                    weights,
                    morphs,
                    indices,
                    material,
                });
            }
        }
        ensure!(
            !primitives.is_empty(),
            "No triangle meshes found in the default scene"
        );
        let mut clips = Vec::new();
        for animation in doc.animations() {
            let mut channels = Vec::new();
            let mut duration = 0.0_f32;
            for channel in animation.channels() {
                let reader = channel.reader(|b| Some(&buffers[b.index()]));
                let times: Vec<_> = reader
                    .read_inputs()
                    .context("Missing animation times")?
                    .collect();
                ensure!(
                    !times.is_empty()
                        && times.iter().all(|t| t.is_finite() && *t >= 0.0)
                        && times.windows(2).all(|w| w[0] < w[1]),
                    "Invalid animation key times"
                );
                let property = channel.target().property();
                let values: Vec<f32> = match reader
                    .read_outputs()
                    .context("Missing animation values")?
                {
                    ReadOutputs::Translations(r) | ReadOutputs::Scales(r) => r.flatten().collect(),
                    ReadOutputs::Rotations(r) => r.into_f32().flatten().collect(),
                    ReadOutputs::MorphTargetWeights(r) => r.into_f32().collect(),
                };
                let width = match property {
                    Property::Rotation => 4,
                    Property::MorphTargetWeights => {
                        nodes[channel.target().node().index()].weights.len()
                    }
                    _ => 3,
                };
                let interpolation = channel.sampler().interpolation();
                ensure!(
                    width > 0
                        && values.len()
                            == times.len()
                                * width
                                * if interpolation == Interpolation::CubicSpline {
                                    3
                                } else {
                                    1
                                },
                    "Animation output count mismatch"
                );
                ensure!(
                    values.iter().all(|v| v.is_finite()),
                    "Non-finite animation values"
                );
                duration = duration.max(*times.last().unwrap());
                channels.push(Channel {
                    node: channel.target().node().index(),
                    property,
                    interpolation,
                    times,
                    values,
                    width,
                });
            }
            clips.push(Clip {
                name: animation
                    .name()
                    .map(String::from)
                    .unwrap_or_else(|| format!("Animation {}", animation.index() + 1)),
                duration,
                channels,
            });
        }
        let used_skins: HashSet<_> = primitives.iter().filter_map(|p| p.skin).collect();
        let joint_set: HashSet<_> = used_skins
            .iter()
            .flat_map(|i| skins[*i].joints.iter().copied())
            .collect();
        let mut joint_nodes: Vec<_> = joint_set.iter().copied().collect();
        joint_nodes.sort_unstable();
        let bones = joint_nodes
            .iter()
            .filter_map(|j| {
                let mut parent = nodes[*j].parent;
                while let Some(p) = parent {
                    if joint_set.contains(&p) {
                        return Some((p, *j));
                    }
                    parent = nodes[p].parent;
                }
                None
            })
            .collect();
        warnings.sort();
        warnings.dedup();
        let mut scene = Self {
            name,
            world: vec![Mat4::IDENTITY; nodes.len()],
            nodes,
            order,
            skins,
            primitives,
            images: images
                .into_iter()
                .map(convert_image)
                .collect::<Result<_>>()?,
            textures,
            materials,
            clips,
            bones,
            joint_nodes,
            bounds: Bounds {
                min: Vec3::ZERO,
                max: Vec3::ONE,
            },
            warnings,
        };
        scene.pose(None, 0.0);
        scene.bounds = scene.posed_bounds();
        Ok(scene)
    }

    pub fn pose(&mut self, clip: Option<usize>, time: f32) {
        let mut transforms: Vec<_> = self.nodes.iter().map(|n| n.base).collect();
        let mut changed = vec![false; self.nodes.len()];
        let mut morph_weights: Vec<_> = self.nodes.iter().map(|n| n.weights.clone()).collect();
        if let Some(clip) = clip.and_then(|i| self.clips.get(i)) {
            for c in &clip.channels {
                let value = c.sample(time);
                match c.property {
                    Property::Translation => {
                        transforms[c.node].translation = Vec3::from_slice(&value)
                    }
                    Property::Rotation => {
                        transforms[c.node].rotation =
                            Quat::from_xyzw(value[0], value[1], value[2], value[3])
                    }
                    Property::Scale => transforms[c.node].scale = Vec3::from_slice(&value),
                    Property::MorphTargetWeights => {
                        morph_weights[c.node] = value;
                        continue;
                    }
                }
                changed[c.node] = true;
            }
        }
        for &i in &self.order {
            let local = if changed[i] {
                transforms[i].matrix()
            } else {
                self.nodes[i].matrix
            };
            self.world[i] = self.nodes[i]
                .parent
                .map_or(local, |p| self.world[p] * local);
        }
        let palettes: Vec<Vec<_>> = self
            .skins
            .iter()
            .map(|s| {
                s.joints
                    .iter()
                    .zip(&s.inverse_bind)
                    .map(|(j, inverse)| {
                        let matrix = self.world[*j] * *inverse;
                        (matrix, normal_matrix(matrix))
                    })
                    .collect()
            })
            .collect();
        for p in &mut self.primitives {
            let world = self.world[p.node];
            let normal = normal_matrix(world);
            for i in 0..p.positions.len() {
                let mut pos = p.positions[i];
                let mut norm = p.normals[i];
                for (m, weight) in p.morphs.iter().zip(&morph_weights[p.node]) {
                    pos += m.positions[i] * *weight;
                    norm += m.normals[i] * *weight;
                }
                let (position, normal) = if let Some(s) = p.skin {
                    let mut position = Vec3::ZERO;
                    let mut normal = Vec3::ZERO;
                    for k in 0..8 {
                        let weight = p.weights[i][k];
                        if weight > 0.0 {
                            let (m, n) = palettes[s][p.joints[i][k] as usize];
                            position += m.transform_point3(pos) * weight;
                            normal += n.transform_vector3(norm) * weight;
                        }
                    }
                    (position, normal)
                } else {
                    (world.transform_point3(pos), normal.transform_vector3(norm))
                };
                p.posed_positions[i] = position;
                p.posed_normals[i] = normal.normalize_or_zero();
            }
        }
    }
    pub fn posed_bounds(&self) -> Bounds {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for p in &self.primitives {
            for &v in &p.posed_positions {
                min = min.min(v);
                max = max.max(v);
            }
        }
        Bounds { min, max }
    }
    pub fn triangles(&self) -> usize {
        self.primitives.iter().map(|p| p.indices.len() / 3).sum()
    }
    pub fn vertices(&self) -> usize {
        self.primitives.iter().map(|p| p.positions.len()).sum()
    }
}

fn normal_matrix(matrix: Mat4) -> Mat4 {
    if matrix.determinant().abs() > 1e-12 {
        matrix.inverse().transpose()
    } else {
        Mat4::IDENTITY
    }
}
fn read_material(m: gltf::Material<'_>, warnings: &mut Vec<String>) -> Surface {
    let pbr = m.pbr_metallic_roughness();
    let texcoord = pbr
        .base_color_texture()
        .map(|t| t.tex_coord())
        .or_else(|| m.normal_texture().map(|t| t.tex_coord()))
        .unwrap_or(0);
    let coords = [
        pbr.metallic_roughness_texture().map(|t| t.tex_coord()),
        m.normal_texture().map(|t| t.tex_coord()),
        m.occlusion_texture().map(|t| t.tex_coord()),
        m.emissive_texture().map(|t| t.tex_coord()),
    ];
    if coords.into_iter().flatten().any(|c| c != texcoord) {
        warnings.push(
            "Material uses multiple UV sets; auxiliary maps with different UVs are disabled".into(),
        );
    }
    Surface {
        base_color: Vec4::from(pbr.base_color_factor()),
        base_texture: pbr.base_color_texture().map(|t| t.texture().index()),
        mr_texture: pbr
            .metallic_roughness_texture()
            .filter(|t| t.tex_coord() == texcoord)
            .map(|t| t.texture().index()),
        normal_texture: m
            .normal_texture()
            .filter(|t| t.tex_coord() == texcoord)
            .map(|t| t.texture().index()),
        ao_texture: m
            .occlusion_texture()
            .filter(|t| t.tex_coord() == texcoord)
            .map(|t| t.texture().index()),
        emissive_texture: m
            .emissive_texture()
            .filter(|t| t.tex_coord() == texcoord)
            .map(|t| t.texture().index()),
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        emissive: Vec3::from(m.emissive_factor()),
        normal_scale: m.normal_texture().map_or(1.0, |t| t.scale()),
        ao_strength: m.occlusion_texture().map_or(1.0, |t| t.strength()),
        alpha_cutoff: if m.alpha_mode() == gltf::material::AlphaMode::Mask {
            m.alpha_cutoff().unwrap_or(0.5)
        } else {
            -1.0
        },
        blend: m.alpha_mode() == gltf::material::AlphaMode::Blend,
        double_sided: m.double_sided(),
        unlit: m.unlit(),
        texcoord,
    }
}
fn triangulate(input: &[u32], mode: Mode) -> Result<Vec<u32>> {
    match mode {
        Mode::Triangles => {
            ensure!(input.len().is_multiple_of(3), "Incomplete triangle indices");
            Ok(input.to_vec())
        }
        Mode::TriangleStrip => Ok(input
            .windows(3)
            .enumerate()
            .flat_map(|(i, t)| {
                if i % 2 == 0 {
                    [t[0], t[1], t[2]]
                } else {
                    [t[1], t[0], t[2]]
                }
            })
            .collect()),
        Mode::TriangleFan => Ok((1..input.len().saturating_sub(1))
            .flat_map(|i| [input[0], input[i], input[i + 1]])
            .collect()),
        _ => bail!("Unsupported primitive topology"),
    }
}
fn generate_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::ZERO; positions.len()];
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
        let normal = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
        for i in [a, b, c] {
            normals[i] += normal;
        }
    }
    normals.iter().map(|n| n.normalize_or_zero()).collect()
}
fn convert_image(image: gltf::image::Data) -> Result<ImageData> {
    use gltf::image::Format::*;
    ensure!(
        image.width > 0 && image.height > 0 && image.width <= 16384 && image.height <= 16384,
        "Texture exceeds 16384 pixels or is empty"
    );
    let (channels, bytes, float) = match image.format {
        R8 => (1, 1, false),
        R8G8 => (2, 1, false),
        R8G8B8 => (3, 1, false),
        R8G8B8A8 => (4, 1, false),
        R16 => (1, 2, false),
        R16G16 => (2, 2, false),
        R16G16B16 => (3, 2, false),
        R16G16B16A16 => (4, 2, false),
        R32G32B32FLOAT => (3, 4, true),
        R32G32B32A32FLOAT => (4, 4, true),
    };
    let mut rgba = Vec::with_capacity(image.width as usize * image.height as usize * 4);
    for pixel in image.pixels.chunks_exact(channels * bytes) {
        let mut c = [255; 4];
        for i in 0..channels {
            c[i] = if float {
                (f32::from_ne_bytes(pixel[i * 4..i * 4 + 4].try_into()?).clamp(0.0, 1.0) * 255.0)
                    .round() as u8
            } else if bytes == 2 {
                (u16::from_ne_bytes(pixel[i * 2..i * 2 + 2].try_into()?) / 257) as u8
            } else {
                pixel[i]
            };
        }
        if channels == 1 {
            c = [c[0], c[0], c[0], 255];
        }
        if channels == 2 {
            c = [c[0], c[0], c[0], c[1]];
        }
        rgba.extend_from_slice(&c);
    }
    Ok(ImageData {
        width: image.width as u16,
        height: image.height as u16,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_winding_is_consistent() {
        assert_eq!(
            triangulate(&[0, 1, 2, 3], Mode::TriangleStrip).unwrap(),
            vec![0, 1, 2, 2, 1, 3]
        );
    }
    #[test]
    fn demo_has_texture_rig_and_animated_skin() {
        let mut scene = Scene::demo().unwrap();
        assert!(!scene.images.is_empty() && !scene.bones.is_empty() && scene.clips.len() >= 2);
        scene.pose(Some(0), 0.0);
        let original: Vec<_> = scene
            .primitives
            .iter()
            .flat_map(|p| p.posed_positions.clone())
            .collect();
        scene.pose(Some(0), 0.75);
        let posed: Vec<_> = scene
            .primitives
            .iter()
            .flat_map(|p| p.posed_positions.clone())
            .collect();
        assert!(
            original
                .iter()
                .zip(&posed)
                .any(|(a, b)| a.distance(*b) > 0.01)
        );
        assert!(posed.iter().all(|p| p.is_finite()));
        scene.pose(None, 0.0);
        assert!(scene.posed_bounds().radius().is_finite());
    }
}
