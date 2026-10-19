//! 3D scéna projektu bez závislosti od UI (plan/architecture.md §6).
//!
//! - [`Scene::build`]: model → trojuholníky plôch, hrany, obálky dielov a popisy skriniek
//!   vo svetových súradniciach (mm, `z` nahor).
//! - [`Camera`] a [`Projection`]: pohľad, premietnutie do obrazu, matica pre GPU a lúč
//!   z bodu obrazu pre výber dielu ([`Scene::pick`]).
//! - [`render`]: softvérový raster so z-bufferom pre snímky bez okna (`ui.render`,
//!   `cabinetcraft render`) a pre okno bez GPU.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

mod camera;
mod raster;
mod scene;

pub use camera::{Camera, Projection};
pub use raster::{render, write_png, Image};
pub use scene::{role_color, Line, PartBox, Scene, SceneOptions, Selection, Tri};

/// Farby zvýraznenia (spoločné pre CPU raster a GPU, aby sa obrazy zhodovali).
pub mod style {
    /// Farba plochy vybraného dielu (zmieša sa s farbou dielu).
    pub const HIGHLIGHT: [f64; 3] = [1.0, 0.62, 0.20];
    /// Podiel farby zvýraznenia.
    pub const HIGHLIGHT_MIX: f64 = 0.55;
    /// Hrany.
    pub const EDGE: [f64; 3] = [0.30, 0.30, 0.33];
    /// Hrany vybraného dielu.
    pub const EDGE_SELECTED: [f64; 3] = [0.75, 0.35, 0.05];
    /// Drôtený obrys vybranej skrinky (kreslí sa navrch, bez hĺbky).
    pub const OUTLINE: [f64; 3] = [0.0, 0.784, 0.902];
    /// Hrúbka obrysu skrinky v bodoch obrazovky.
    pub const OUTLINE_WIDTH: f64 = 2.0;
    /// Smer svetla v súradniciach pohľadu (vodorovne, nahor, hĺbka); zľava zhora spredu.
    pub const LIGHT: [f64; 3] = [-0.35, 0.6, -0.72];
    /// Tieňovanie: `AMBIENT + DIFFUSE · max(n·l, 0)`.
    pub const AMBIENT: f64 = 0.58;
    pub const DIFFUSE: f64 = 0.42;
    /// Hrany sa kreslia o toľko mm bližšie k oku (aby ich nezakryla vlastná plocha).
    pub const EDGE_BIAS_MM: f64 = 1.5;
    /// Otvory a drážky (ležia 0,4 mm nad plochou) dostanú ešte toto zvýhodnenie v hĺbke.
    pub const MARK_BIAS_MM: f64 = 0.3;
    /// Stena sa skryje, keď je jej normála (do miestnosti) odvrátená od oka viac ako toto.
    pub const WALL_CULL: f64 = 0.05;
    /// Pozadie svetlého a tmavého vzhľadu.
    pub const BACKGROUND_LIGHT: [u8; 3] = [246, 247, 249];
    pub const BACKGROUND_DARK: [u8; 3] = [32, 34, 38];
}
