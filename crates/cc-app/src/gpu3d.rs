//! 3D výrez na GPU: wgpu cez `egui_wgpu::CallbackTrait` (ADR 0001).
//!
//! Scénu z `cc-scene` nahrá raz do vertex bufferov (pri zmene modelu alebo volieb). V každej
//! snímke, keď sa zmení pohľad alebo výber, ju v `prepare` vykreslí do vlastného obrazu
//! (MSAA 4×, hĺbkový buffer) a v `paint` ho prenesie do výrezu okna. Vzorce tieňovania sú
//! rovnaké ako v softvérovom rastri, takže `ui.render` sa zhoduje s oknom.
//!
//! Chyba wgpu sa neprenesie ako pád: zachytí ju vlastná obsluha a výrez sa prepne na CPU
//! raster.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cc_scene::{style, Projection, Scene};
use eframe::egui_wgpu::{self, wgpu};
use wgpu::util::DeviceExt;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Bajty na vrchol: poloha 3×f32, normála 3×f32, farba 4×f32, diel u32, stena 3×f32.
const VERTEX_SIZE: usize = 56;
const UNIFORM_SIZE: usize = 240;

/// Vrcholy scény pripravené na nahratie (bez závislosti od zariadenia).
pub(crate) struct Mesh {
    generation: u64,
    tris: Vec<u8>,
    tri_vertices: u32,
    lines: Vec<u8>,
    line_vertices: u32,
}

fn push_f32(out: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn vec3(v: glam::DVec3) -> [f32; 3] {
    [v.x as f32, v.y as f32, v.z as f32]
}

#[allow(clippy::too_many_arguments)]
fn push_vertex(
    out: &mut Vec<u8>,
    pos: glam::DVec3,
    normal: glam::DVec3,
    color: [f64; 3],
    lit: bool,
    part: u32,
    cull: Option<glam::DVec3>,
) {
    push_f32(out, &vec3(pos));
    push_f32(out, &vec3(normal));
    push_f32(
        out,
        &[
            color[0] as f32,
            color[1] as f32,
            color[2] as f32,
            if lit { 1.0 } else { 0.0 },
        ],
    );
    out.extend_from_slice(&part.to_le_bytes());
    push_f32(out, &vec3(cull.unwrap_or(glam::DVec3::ZERO)));
}

impl Mesh {
    pub(crate) fn build(scene: &Scene, generation: u64) -> Mesh {
        let mut tris = Vec::with_capacity(scene.tris().len() * 3 * VERTEX_SIZE);
        for t in scene.tris() {
            for p in t.p {
                push_vertex(&mut tris, p, t.n, t.color, t.lit, t.part, t.cull);
            }
        }
        let mut lines = Vec::with_capacity(scene.lines().len() * 2 * VERTEX_SIZE);
        for l in scene.lines() {
            for p in [l.a, l.b] {
                push_vertex(
                    &mut lines,
                    p,
                    glam::DVec3::ZERO,
                    style::EDGE,
                    true,
                    l.part,
                    l.cull,
                );
            }
        }
        Mesh {
            generation,
            tri_vertices: (scene.tris().len() * 3) as u32,
            line_vertices: (scene.lines().len() * 2) as u32,
            tris,
            lines,
        }
    }

    pub(crate) fn triangles(&self) -> usize {
        self.tri_vertices as usize / 3
    }
}

/// Uniformy jednej snímky (rozloženie ako `Uniforms` vo `view3d.wgsl`).
fn uniforms(proj: &Projection, selected: Option<u32>) -> Vec<u8> {
    let mut out = Vec::with_capacity(UNIFORM_SIZE);
    for col in proj.clip_matrix() {
        push_f32(&mut out, &col);
    }
    for row in proj.rotation_rows() {
        push_f32(
            &mut out,
            &[row[0] as f32, row[1] as f32, row[2] as f32, 0.0],
        );
    }
    let eye = match proj.eye() {
        Some(e) => [e.x as f32, e.y as f32, e.z as f32, 1.0],
        None => {
            let d = proj.view_dir();
            [d.x as f32, d.y as f32, d.z as f32, 0.0]
        }
    };
    push_f32(&mut out, &eye);
    let l = glam::DVec3::from_array(style::LIGHT).normalize();
    push_f32(
        &mut out,
        &[l.x as f32, l.y as f32, l.z as f32, style::AMBIENT as f32],
    );
    push_f32(
        &mut out,
        &[
            style::DIFFUSE as f32,
            style::HIGHLIGHT_MIX as f32,
            style::EDGE_BIAS_MM as f32,
            style::MARK_BIAS_MM as f32,
        ],
    );
    for c in [style::HIGHLIGHT, style::EDGE, style::EDGE_SELECTED] {
        push_f32(&mut out, &[c[0] as f32, c[1] as f32, c[2] as f32, 1.0]);
    }
    for v in [selected.unwrap_or(0), 0, 0, 0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    push_f32(&mut out, &[style::WALL_CULL as f32, 0.0, 0.0, 0.0]);
    out
}

/// Pipelines a buffery; žijú v `callback_resources` renderera egui.
struct Resources {
    tri_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    uniform_group: wgpu::BindGroup,
    samples: u32,
    mesh: Option<GpuMesh>,
    targets: Option<Targets>,
    /// kľúč posledného vykresleného obrazu (pohľad, výber, scéna, veľkosť)
    last_key: Option<u64>,
}

struct GpuMesh {
    generation: u64,
    tris: Option<wgpu::Buffer>,
    tri_vertices: u32,
    lines: Option<wgpu::Buffer>,
    line_vertices: u32,
}

struct Targets {
    size: [u32; 2],
    msaa: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
    resolved: wgpu::TextureView,
    blit_group: wgpu::BindGroup,
}

/// GPU je k dispozícii (zariadenie z eframe a pripravené pipelines).
#[derive(Clone)]
pub(crate) struct Gpu {
    failed: Arc<AtomicBool>,
    pub(crate) adapter: String,
    max_size: u32,
}

impl Gpu {
    /// Pripraví pipelines; `None`, ak ich zariadenie nevie vytvoriť.
    pub(crate) fn new(rs: &egui_wgpu::RenderState) -> Option<Gpu> {
        let failed = Arc::new(AtomicBool::new(false));
        {
            let failed = failed.clone();
            rs.device.on_uncaptured_error(Box::new(move |e| {
                eprintln!("wgpu: {e}");
                failed.store(true, Ordering::Relaxed);
            }));
        }
        let samples = if rs
            .adapter
            .get_texture_format_features(COLOR_FORMAT)
            .flags
            .sample_count_supported(4)
            && rs
                .adapter
                .get_texture_format_features(DEPTH_FORMAT)
                .flags
                .sample_count_supported(4)
        {
            4
        } else {
            1
        };
        let resources = Resources::new(&rs.device, rs.target_format, samples);
        if failed.load(Ordering::Relaxed) {
            return None;
        }
        rs.renderer.write().callback_resources.insert(resources);
        let info = rs.adapter.get_info();
        Some(Gpu {
            failed,
            adapter: format!("{} ({:?}, MSAA {samples}×)", info.name, info.backend),
            max_size: rs.device.limits().max_texture_dimension_2d,
        })
    }

    /// GPU nahlásilo chybu: výrez prejde na CPU raster.
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Kreslenie výrezu: pridá do `painter` callback, ktorý scénu vykreslí na GPU.
    pub(crate) fn paint(
        &self,
        painter: &eframe::egui::Painter,
        rect: eframe::egui::Rect,
        size: [usize; 2],
        frame: Frame3d,
    ) -> bool {
        let max = self.max_size as usize;
        if size[0] == 0 || size[1] == 0 || size[0] > max || size[1] > max {
            return false;
        }
        painter.add(egui_wgpu::Callback::new_paint_callback(rect, frame));
        true
    }
}

/// Jedna snímka výrezu (callback pre egui-wgpu).
pub(crate) struct Frame3d {
    pub mesh: Arc<Mesh>,
    pub size: [u32; 2],
    pub key: u64,
    pub uniforms: Vec<u8>,
    pub background: [u8; 3],
}

impl Frame3d {
    pub(crate) fn new(
        mesh: Arc<Mesh>,
        proj: &Projection,
        size: [usize; 2],
        selected: Option<u32>,
        background: [u8; 3],
        key: u64,
    ) -> Frame3d {
        Frame3d {
            mesh,
            size: [size[0] as u32, size[1] as u32],
            key,
            uniforms: uniforms(proj, selected),
            background,
        }
    }
}

impl Resources {
    fn new(device: &wgpu::Device, target: wgpu::TextureFormat, samples: u32) -> Resources {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cc view3d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("view3d.wgsl").into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cc blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blit.wgsl").into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cc view3d uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cc view3d uniforms"),
            size: UNIFORM_SIZE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cc view3d uniforms"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cc view3d"),
            bind_group_layouts: &[&uniform_layout],
            push_constant_ranges: &[],
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
            3 => Uint32,
            4 => Float32x3,
        ];
        let buffers = [wgpu::VertexBufferLayout {
            array_stride: VERTEX_SIZE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        }];
        let scene_pipeline = |label, entry, topology, compare| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: compare,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let tri_pipeline = scene_pipeline(
            "cc view3d plochy",
            "vs_tri",
            wgpu::PrimitiveTopology::TriangleList,
            wgpu::CompareFunction::Less,
        );
        let line_pipeline = scene_pipeline(
            "cc view3d hrany",
            "vs_line",
            wgpu::PrimitiveTopology::LineList,
            wgpu::CompareFunction::LessEqual,
        );

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cc blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cc blit"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cc blit"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_blit"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_blit"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cc blit"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Resources {
            tri_pipeline,
            line_pipeline,
            blit_pipeline,
            blit_layout,
            sampler,
            uniform,
            uniform_group,
            samples,
            mesh: None,
            targets: None,
            last_key: None,
        }
    }

    fn upload_mesh(&mut self, device: &wgpu::Device, mesh: &Mesh) {
        if self
            .mesh
            .as_ref()
            .is_some_and(|m| m.generation == mesh.generation)
        {
            return;
        }
        let buffer = |label, data: &[u8]| {
            (!data.is_empty()).then(|| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: data,
                    usage: wgpu::BufferUsages::VERTEX,
                })
            })
        };
        self.mesh = Some(GpuMesh {
            generation: mesh.generation,
            tris: buffer("cc view3d plochy", &mesh.tris),
            tri_vertices: mesh.tri_vertices,
            lines: buffer("cc view3d hrany", &mesh.lines),
            line_vertices: mesh.line_vertices,
        });
        self.last_key = None;
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let texture = |label, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let resolved = texture(
            "cc view3d obraz",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let msaa = (self.samples > 1).then(|| {
            texture(
                "cc view3d msaa",
                COLOR_FORMAT,
                self.samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        let depth = texture(
            "cc view3d hĺbka",
            DEPTH_FORMAT,
            self.samples,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let blit_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cc blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&resolved),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.targets = Some(Targets {
            size,
            msaa,
            depth,
            resolved,
            blit_group,
        });
        self.last_key = None;
    }
}

impl egui_wgpu::CallbackTrait for Frame3d {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get_mut::<Resources>() else {
            return Vec::new();
        };
        r.upload_mesh(device, &self.mesh);
        r.ensure_targets(device, self.size);
        if r.last_key == Some(self.key) {
            return Vec::new(); // obraz sa nezmenil
        }
        r.last_key = Some(self.key);
        queue.write_buffer(&r.uniform, 0, &self.uniforms);
        let (Some(targets), Some(mesh)) = (&r.targets, &r.mesh) else {
            return Vec::new();
        };
        let bg = self.background.map(|c| c as f64 / 255.0);
        let (view, resolve) = match &targets.msaa {
            Some(msaa) => (msaa, Some(&targets.resolved)),
            None => (&targets.resolved, None),
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("cc view3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: resolve,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: bg[0],
                        g: bg[1],
                        b: bg[2],
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &r.uniform_group, &[]);
        if let Some(buffer) = &mesh.tris {
            pass.set_pipeline(&r.tri_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..mesh.tri_vertices, 0..1);
        }
        if let Some(buffer) = &mesh.lines {
            pass.set_pipeline(&r.line_pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..mesh.line_vertices, 0..1);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(r) = resources.get::<Resources>() else {
            return;
        };
        let Some(targets) = &r.targets else {
            return;
        };
        pass.set_pipeline(&r.blit_pipeline);
        pass.set_bind_group(0, &targets.blit_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_layout_matches_shader() {
        let scene = Scene::default();
        let proj = Projection::new(&scene, &Default::default(), 100.0, 100.0, 1.0);
        assert_eq!(uniforms(&proj, Some(3)).len(), UNIFORM_SIZE);
    }

    #[test]
    fn mesh_has_vertex_per_corner() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let loaded = cc_config::load(&root.join("examples/dielna/kuchyna.toml")).unwrap();
        let model = cc_pipeline::build(&loaded.project, &loaded.standard, &loaded.materials, None)
            .unwrap()
            .model;
        let scene = Scene::build(&model, &loaded.project, &Default::default());
        let mesh = Mesh::build(&scene, 1);
        assert_eq!(mesh.tris.len(), scene.tris().len() * 3 * VERTEX_SIZE);
        assert_eq!(mesh.lines.len(), scene.lines().len() * 2 * VERTEX_SIZE);
    }

    /// Shader sa preloží (naga) bez GPU.
    #[test]
    fn shaders_parse() {
        for src in [include_str!("view3d.wgsl"), include_str!("blit.wgsl")] {
            let module = naga::front::wgsl::parse_str(src).unwrap();
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap();
        }
    }
}
