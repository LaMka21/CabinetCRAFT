use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Hrana dielu podľa polohy v súradniciach dielu (kap. 3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Edge {
    X0,
    X1,
    Y0,
    Y1,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::X0, Edge::X1, Edge::Y0, Edge::Y1];

    /// Vonkajšia normála hrany v súradniciach dielu.
    pub fn outward_normal(self) -> DVec3 {
        match self {
            Edge::X0 => DVec3::NEG_X,
            Edge::X1 => DVec3::X,
            Edge::Y0 => DVec3::NEG_Y,
            Edge::Y1 => DVec3::Y,
        }
    }
}

/// Transformácia zo súradníc dielu do súradníc skrinky (kap. 3.2).
///
/// Osi dielu sú vyjadrené v súradniciach skrinky a musia tvoriť pravotočivú
/// ortonormálnu sústavu. V MVP sú všetky osi rovnobežné s osami skrinky.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub x_axis: DVec3,
    pub y_axis: DVec3,
    pub z_axis: DVec3,
    pub origin: DVec3,
}

impl Transform {
    /// Vytvorí transformáciu z osí `x`, `y` a počiatku; os `z = x × y`.
    pub fn from_axes(x_axis: DVec3, y_axis: DVec3, origin: DVec3) -> Self {
        // `+ 0.0` mení −0.0 na 0.0, aby bol serializovaný výstup čistý.
        Transform {
            x_axis: x_axis + 0.0,
            y_axis: y_axis + 0.0,
            z_axis: x_axis.cross(y_axis) + 0.0,
            origin: origin + 0.0,
        }
    }

    /// Bod v súradniciach dielu → bod v súradniciach skrinky.
    pub fn point_to_cabinet(&self, p: DVec3) -> DVec3 {
        self.origin + self.x_axis * p.x + self.y_axis * p.y + self.z_axis * p.z
    }

    /// Smer v súradniciach dielu → smer v súradniciach skrinky.
    pub fn dir_to_cabinet(&self, v: DVec3) -> DVec3 {
        self.x_axis * v.x + self.y_axis * v.y + self.z_axis * v.z
    }

    /// Bod v súradniciach skrinky → bod v súradniciach dielu.
    pub fn point_to_part(&self, p: DVec3) -> DVec3 {
        let d = p - self.origin;
        DVec3::new(d.dot(self.x_axis), d.dot(self.y_axis), d.dot(self.z_axis))
    }

    /// Smer v súradniciach skrinky → smer v súradniciach dielu.
    pub fn dir_to_part(&self, v: DVec3) -> DVec3 {
        DVec3::new(v.dot(self.x_axis), v.dot(self.y_axis), v.dot(self.z_axis))
    }

    /// True, ak sú osi ortonormálne a pravotočivé.
    pub fn is_right_handed_orthonormal(&self) -> bool {
        let tol = 1e-9;
        (self.x_axis.length() - 1.0).abs() < tol
            && (self.y_axis.length() - 1.0).abs() < tol
            && self.x_axis.dot(self.y_axis).abs() < tol
            && (self.x_axis.cross(self.y_axis) - self.z_axis).length() < tol
    }

    /// Hrana dielu, ktorej vonkajšia normála v skrinke má smer `dir`.
    pub fn edge_facing(&self, dir: DVec3) -> Option<Edge> {
        Edge::ALL
            .into_iter()
            .find(|e| self.dir_to_cabinet(e.outward_normal()).dot(dir) > 1.0 - 1e-9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn roundtrip_point() {
        let t = Transform::from_axes(DVec3::NEG_Z, DVec3::Y, DVec3::new(0.0, 0.0, 720.0));
        assert!(t.is_right_handed_orthonormal());
        assert_eq!(t.z_axis, DVec3::X);
        let p = DVec3::new(100.0, 50.0, 18.0);
        let back = t.point_to_part(t.point_to_cabinet(p));
        assert_relative_eq!(back.x, p.x);
        assert_relative_eq!(back.y, p.y);
        assert_relative_eq!(back.z, p.z);
    }

    #[test]
    fn edge_facing_front() {
        let t = Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO);
        assert_eq!(t.edge_facing(DVec3::NEG_Y), Some(Edge::Y0));
        assert_eq!(t.edge_facing(DVec3::X), Some(Edge::X1));
        assert_eq!(t.edge_facing(DVec3::Z), None);
    }
}
