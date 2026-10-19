use serde::{Deserialize, Serialize};

/// Strana obrábania dielu (kap. 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Face {
    F,
    B,
    X0,
    X1,
    Y0,
    Y1,
}

/// Bod v rovine plochy dielu.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct P2 {
    pub x: f64,
    pub y: f64,
}

/// Operácia v súradniciach dielu (kap. 6.4). Generujú sa až v M2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    Hole {
        face: Face,
        x: f64,
        y: f64,
        z: f64,
        diameter: f64,
        depth: f64,
        through: bool,
        dowel: bool,
    },
    Groove {
        face: Face,
        start: P2,
        end: P2,
        width: f64,
        depth: f64,
    },
}
