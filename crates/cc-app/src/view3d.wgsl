// 3D výrez CabinetCRAFT: plochy s jednoduchým osvetlením, hrany, zvýraznenie výberu.
// Vzorce zodpovedajú softvérovému rastru v cc-scene (raster.rs), aby sa obrazy zhodovali.

struct Uniforms {
    clip: mat4x4<f32>,
    // riadky otočenia svet → pohľad (vodorovne, nahor, hĺbka)
    rot0: vec4<f32>,
    rot1: vec4<f32>,
    rot2: vec4<f32>,
    // xyz: oko (perspektíva, w = 1) alebo smer pohľadu (ortogonálne, w = 0)
    eye: vec4<f32>,
    // xyz: svetlo v súradniciach pohľadu, w: ambient
    light: vec4<f32>,
    // x: diffuse, y: podiel zvýraznenia, z: posun hrán k oku (mm), w: posun otvorov (mm)
    params: vec4<f32>,
    highlight: vec4<f32>,
    edge: vec4<f32>,
    edge_selected: vec4<f32>,
    // x: vybraný diel (index + 1, 0 = nič)
    selection: vec4<u32>,
    // x: prah skrytia steny
    params2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // rgb, a = 1 osvetlená plocha, 0 otvor/drážka
    @location(2) color: vec4<f32>,
    @location(3) part: u32,
    // normála steny do miestnosti (0 = nie je stena)
    @location(4) cull: vec3<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) facing: f32,
};

fn view_rot(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(u.rot0.xyz, v), dot(u.rot1.xyz, v), dot(u.rot2.xyz, v));
}

fn toward_eye(p: vec3<f32>) -> vec3<f32> {
    if (u.eye.w > 0.5) {
        return normalize(u.eye.xyz - p);
    }
    return -u.eye.xyz;
}

fn wall_hidden(c: vec3<f32>) -> bool {
    return dot(c, c) > 0.0 && view_rot(c).z > u.params2.x;
}

fn selected(part: u32) -> bool {
    return part != 0u && part == u.selection.x;
}

/// Bod mimo orezového objemu: celý trojuholník/hrana sa zahodí.
const CLIPPED: vec4<f32> = vec4<f32>(0.0, 0.0, -2.0, 1.0);

@vertex
fn vs_tri(v: VertexIn) -> VertexOut {
    var o: VertexOut;
    let lit = v.color.a > 0.5;
    var p = v.pos;
    if (!lit) {
        p = p + toward_eye(p) * u.params.w;
    }
    o.clip = u.clip * vec4<f32>(p, 1.0);
    if (wall_hidden(v.cull)) {
        o.clip = CLIPPED;
    }
    var c = v.color.rgb;
    var shade = 1.0;
    if (lit) {
        if (selected(v.part)) {
            c = mix(c, u.highlight.rgb, u.params.y);
        }
        shade = u.light.w + u.params.x * max(dot(view_rot(v.normal), u.light.xyz), 0.0);
    }
    o.color = min(c * shade, vec3<f32>(1.0));
    // plocha natočená k oku (rovnako ako `Projection::facing`)
    if (u.eye.w > 0.5) {
        o.facing = dot(v.normal, u.eye.xyz - v.pos);
    } else {
        o.facing = -dot(v.normal, u.eye.xyz);
    }
    return o;
}

@vertex
fn vs_line(v: VertexIn) -> VertexOut {
    var o: VertexOut;
    let p = v.pos + toward_eye(v.pos) * u.params.z;
    o.clip = u.clip * vec4<f32>(p, 1.0);
    if (wall_hidden(v.cull)) {
        o.clip = CLIPPED;
    }
    o.color = select(u.edge.rgb, u.edge_selected.rgb, selected(v.part));
    o.facing = 1.0;
    return o;
}

@fragment
fn fs_main(i: VertexOut) -> @location(0) vec4<f32> {
    if (i.facing < 0.0) {
        discard;
    }
    return vec4<f32>(i.color, 1.0);
}
