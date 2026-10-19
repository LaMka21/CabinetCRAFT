use serde::{Deserialize, Serialize};

/// Smer vlákna/dekoru vzhľadom na dĺžku dielu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grain {
    #[default]
    None,
    Length,
    Width,
}

impl Grain {
    pub fn as_str(self) -> &'static str {
        match self {
            Grain::None => "none",
            Grain::Length => "length",
            Grain::Width => "width",
        }
    }
}

/// Doskový materiál (kap. 4.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub id: String,
    pub thickness: f64,
    #[serde(default)]
    pub grain: Grain,
    /// Predvolené olepenie viditeľných hrán.
    #[serde(default)]
    pub edge_band: Option<String>,
}

/// Hranová páska (kap. 4.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeBand {
    pub id: String,
    pub thickness: f64,
}

/// Obsah súboru `materials.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialLibrary {
    #[serde(default, rename = "material")]
    pub materials: Vec<Material>,
    #[serde(default, rename = "edge_band")]
    pub edge_bands: Vec<EdgeBand>,
}

impl MaterialLibrary {
    pub fn material(&self, id: &str) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }

    pub fn edge_band(&self, id: &str) -> Option<&EdgeBand> {
        self.edge_bands.iter().find(|e| e.id == id)
    }
}
