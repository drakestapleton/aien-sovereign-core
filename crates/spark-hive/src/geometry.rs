use serde::{Deserialize, Serialize};

pub const DEFAULT_HEX_RADIUS: f64 = 88.0;
pub const SQRT_3: f64 = 1.732050807568877293527446341505872367;

/// 6 Axial Directions:
/// [1, 0] East, [1, -1] Northeast, [0, -1] Northwest, [-1, 0] West, [-1, 1] Southwest, [0, 1] Southeast
pub const HEX_DIRECTIONS: [(i32, i32); 6] = [
    (1, 0),   // East
    (1, -1),  // Northeast
    (0, -1),  // Northwest
    (-1, 0),  // West
    (-1, 1),  // Southwest
    (0, 1),   // Southeast
];

pub const HEX_DIRECTION_LABELS: [&str; 6] = [
    "east",
    "northeast",
    "northwest",
    "west",
    "southwest",
    "southeast",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HexCoord {
    pub q: i32,
    pub r: i32,
}

impl HexCoord {
    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    pub fn distance(&self, other: HexCoord) -> i32 {
        let dq = self.q - other.q;
        let dr = self.r - other.r;
        dq.abs().max(dr.abs()).max((dq + dr).abs())
    }

    pub fn neighbors(&self) -> [HexCoord; 6] {
        [
            HexCoord::new(self.q + 1, self.r),
            HexCoord::new(self.q + 1, self.r - 1),
            HexCoord::new(self.q, self.r - 1),
            HexCoord::new(self.q - 1, self.r),
            HexCoord::new(self.q - 1, self.r + 1),
            HexCoord::new(self.q, self.r + 1),
        ]
    }

    pub fn to_pixel(&self, radius: f64) -> (f64, f64) {
        axial_to_pixel(self.q, self.r, radius)
    }

    pub fn from_pixel(x: f64, y: f64, radius: f64) -> Self {
        pixel_to_axial(x, y, radius)
    }
}

pub fn neighbors(q: i32, r: i32) -> [HexCoord; 6] {
    HexCoord::new(q, r).neighbors()
}

pub fn axial_to_pixel(q: i32, r: i32, radius: f64) -> (f64, f64) {
    let x = radius * SQRT_3 * (q as f64 + (r as f64) / 2.0);
    let y = radius * 1.5 * (r as f64);
    (x, y)
}

pub fn round_axial(cube_x: f64, cube_z: f64) -> HexCoord {
    let cube_y = -cube_x - cube_z;
    let mut rx = cube_x.round();
    let ry = cube_y.round();
    let mut rz = cube_z.round();

    let dx = (rx - cube_x).abs();
    let dy = (ry - cube_y).abs();
    let dz = (rz - cube_z).abs();

    if dx > dy && dx > dz {
        rx = -ry - rz;
    } else if dz > dy {
        rz = -rx - ry;
    }

    HexCoord::new(rx as i32, rz as i32)
}

pub fn pixel_to_axial(x: f64, y: f64, radius: f64) -> HexCoord {
    let q = (SQRT_3 / 3.0 * x - y / 3.0) / radius;
    let r = (2.0 / 3.0 * y) / radius;
    round_axial(q, r)
}

pub fn hex_distance(q1: i32, r1: i32, q2: i32, r2: i32) -> i32 {
    let dq = q1 - q2;
    let dr = r1 - r2;
    dq.abs().max(dr.abs()).max((dq + dr).abs())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailTier {
    Full,    // >= 0.62 zoom
    Compact, // >= 0.34 zoom
    Dot,     // < 0.34 zoom
}

impl DetailTier {
    pub fn for_zoom(zoom: f64) -> Self {
        if zoom >= 0.62 {
            DetailTier::Full
        } else if zoom >= 0.34 {
            DetailTier::Compact
        } else {
            DetailTier::Dot
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DetailTier::Full => "full",
            DetailTier::Compact => "compact",
            DetailTier::Dot => "dot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementIntent {
    Independent,
    Join,
    Branch,
    Meet,
}

impl PlacementIntent {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "join" => PlacementIntent::Join,
            "branch" => PlacementIntent::Branch,
            "meet" => PlacementIntent::Meet,
            _ => PlacementIntent::Independent,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PlacementIntent::Independent => "independent",
            PlacementIntent::Join => "join",
            PlacementIntent::Branch => "branch",
            PlacementIntent::Meet => "meet",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_directions() {
        assert_eq!(HEX_DIRECTIONS.len(), 6);
        assert_eq!(HEX_DIRECTIONS[0], (1, 0));   // East
        assert_eq!(HEX_DIRECTIONS[1], (1, -1));  // Northeast
        assert_eq!(HEX_DIRECTIONS[2], (0, -1));  // Northwest
        assert_eq!(HEX_DIRECTIONS[3], (-1, 0));  // West
        assert_eq!(HEX_DIRECTIONS[4], (-1, 1));  // Southwest
        assert_eq!(HEX_DIRECTIONS[5], (0, 1));   // Southeast
    }

    #[test]
    fn test_axial_to_pixel_and_back_roundtrip() {
        let test_points = [(0, 0), (1, 0), (1, -1), (-4, 3), (7, -2)];
        for (q, r) in test_points {
            let (x, y) = axial_to_pixel(q, r, DEFAULT_HEX_RADIUS);
            let back = pixel_to_axial(x, y, DEFAULT_HEX_RADIUS);
            assert_eq!(back, HexCoord::new(q, r), "Mismatch for point ({}, {})", q, r);
        }
    }

    #[test]
    fn test_hex_distance() {
        assert_eq!(hex_distance(0, 0, 0, 0), 0);
        assert_eq!(hex_distance(0, 0, 1, 0), 1);
        assert_eq!(hex_distance(0, 0, 1, -1), 1);
        assert_eq!(hex_distance(0, 0, 2, 2), 4);
    }

    #[test]
    fn test_neighbors() {
        let origin = HexCoord::new(3, -2);
        let nbrs = neighbors(3, -2);
        assert_eq!(nbrs.len(), 6);
        for n in nbrs {
            assert_eq!(origin.distance(n), 1);
        }
    }

    #[test]
    fn test_detail_tiers() {
        assert_eq!(DetailTier::for_zoom(1.0), DetailTier::Full);
        assert_eq!(DetailTier::for_zoom(0.62), DetailTier::Full);
        assert_eq!(DetailTier::for_zoom(0.61), DetailTier::Compact);
        assert_eq!(DetailTier::for_zoom(0.34), DetailTier::Compact);
        assert_eq!(DetailTier::for_zoom(0.33), DetailTier::Dot);
        assert_eq!(DetailTier::for_zoom(0.10), DetailTier::Dot);
    }

    #[test]
    fn test_placement_intents() {
        assert_eq!(PlacementIntent::parse("join"), PlacementIntent::Join);
        assert_eq!(PlacementIntent::parse("BRANCH"), PlacementIntent::Branch);
        assert_eq!(PlacementIntent::parse("meet"), PlacementIntent::Meet);
        assert_eq!(PlacementIntent::parse("other"), PlacementIntent::Independent);
    }
}
