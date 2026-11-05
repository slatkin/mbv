//! The semantic hit map (design D6): the regions a Client painted for one
//! frame, each with the semantic target a press there resolves to.

use mbv_ui_msg::Position;

/// A rectangular hit region in `f32` logical units.
///
/// The TUI converts its painted cell rectangles to these units and the GUI
/// reports egui points; the core never sees either unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl HitRegion {
    /// Construct a region from its top-left corner and size.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether `position` lies inside the region. The region is half-open on
    /// both axes: the top and left edges contain, the bottom and right edges
    /// do not, so adjacent regions tile without sharing a point.
    #[must_use]
    pub fn contains(&self, position: Position) -> bool {
        position.x >= self.x
            && position.x < self.x + self.width
            && position.y >= self.y
            && position.y < self.y + self.height
    }
}

/// One frame's semantic hit map: the regions a Client painted, each with the
/// semantic target a pointer press there resolves to.
///
/// The map is an owned snapshot of one frame. A Client replaces it every
/// frame, and pointer resolution between frames uses the map of the last
/// painted frame (`client-interaction-core`: pointer targets resolve from a
/// semantic hit map). The core only tests points against regions; it never
/// computes geometry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HitMap<Target> {
    regions: Vec<(HitRegion, Target)>,
}

impl<Target> HitMap<Target> {
    /// Create an empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Record a painted region and the target a press on it resolves to.
    ///
    /// Later pushes paint on top of earlier ones: when regions overlap, the
    /// last one pushed wins the resolution.
    pub fn push(&mut self, region: HitRegion, target: Target) {
        self.regions.push((region, target));
    }

    /// Number of recorded regions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Whether no region is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Resolve a pointer position against the painted regions. The last
    /// region painted over the position wins. `None` when the position lands
    /// in no region.
    #[must_use]
    pub fn resolve(&self, position: Position) -> Option<&Target> {
        self.regions
            .iter()
            .rev()
            .find(|(region, _)| region.contains(position))
            .map(|(_, target)| target)
    }
}
