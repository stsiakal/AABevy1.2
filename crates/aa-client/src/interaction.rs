//! Surface distances for the original interaction gates.
//!
//! `FUN_39093860` does not compare origins: it accepts when `FUN_3989bf90` returns a value below
//! `sqrt(squared_range)`. `FUN_3989bf90` is the minimum over the target's collision shapes of
//! `FUN_398b1ab0`; for two spheres `FUN_398b1af0` is `|c1 - c2| - (r1 + r2)`. A target with a box
//! shape (a house's model bounds) is measured to its nearest wall the same way. The helpers here
//! take game coordinates (x east, y north, z up) unless a name says Bevy.

use aa_world::bevy_to_cry;
use bevy::camera::primitives::Aabb;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Gap between two spheres, clamped at 0: `FUN_398b1af0`.
pub(crate) fn sphere_gap(from: Vec3, from_radius: f32, to: Vec3, to_radius: f32) -> f32 {
    (from.distance(to) - from_radius.max(0.0) - to_radius.max(0.0)).max(0.0)
}

/// Gap between a sphere and an axis-aligned box, clamped at 0: the nearest point of the box to the
/// sphere's centre minus its radius (the box half of `FUN_398b1ab0`).
pub(crate) fn box_gap(from: Vec3, from_radius: f32, min: Vec3, max: Vec3) -> f32 {
    let (min, max) = (min.min(max), min.max(max));
    let nearest = from.clamp(min, max);
    (from.distance(nearest) - from_radius.max(0.0)).max(0.0)
}

/// A target's interaction shape, in game coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    Sphere { center: Vec3, radius: f32 },
    Box { min: Vec3, max: Vec3 },
}

impl Shape {
    /// A sphere whose radius is the actor model's collision radius.
    pub(crate) fn sphere(center: Vec3, radius: f32) -> Shape {
        Shape::Sphere { center, radius }
    }

    /// A box from a rendered model's world-space (Bevy) bounds, converted to game coordinates.
    pub(crate) fn from_bevy_box(min: Vec3, max: Vec3) -> Shape {
        let (a, b) = (bevy_to_cry(min), bevy_to_cry(max));
        Shape::Box { min: a.min(b), max: a.max(b) }
    }

    /// `FUN_3989bf90`: the surface gap of this shape to the player's sphere.
    pub(crate) fn gap(self, from: Vec3, from_radius: f32) -> f32 {
        match self {
            Shape::Sphere { center, radius } => sphere_gap(from, from_radius, center, radius),
            Shape::Box { min, max } => box_gap(from, from_radius, min, max),
        }
    }
}

/// The world-space bounds of a unit's rendered meshes, for the targets whose interaction shape is a
/// box (a house) or whose actor row has no collision radius (a ship, a wagon).
#[derive(SystemParam)]
pub(crate) struct UnitBounds<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    bounds: Query<'w, 's, (&'static Aabb, &'static GlobalTransform)>,
}

impl UnitBounds<'_, '_> {
    /// The union of `root`'s and its descendants' AABBs, in Bevy world space.
    pub(crate) fn world(&self, root: Entity) -> Option<(Vec3, Vec3)> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut found = false;
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if let Ok((aabb, global)) = self.bounds.get(entity) {
                found = true;
                let c = Vec3::from(aabb.center);
                let h = Vec3::from(aabb.half_extents);
                for sx in [-1.0, 1.0] {
                    for sy in [-1.0, 1.0] {
                        for sz in [-1.0, 1.0] {
                            let p = global.transform_point(c + h * Vec3::new(sx, sy, sz));
                            min = min.min(p);
                            max = max.max(p);
                        }
                    }
                }
            }
            if let Ok(kids) = self.children.get(entity) {
                stack.extend(kids.iter());
            }
        }
        found.then_some((min, max))
    }

    /// The unit's interaction shape from its rendered model bounds.
    pub(crate) fn shape(&self, root: Entity) -> Option<Shape> {
        self.world(root).map(|(min, max)| Shape::from_bevy_box(min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spheres_measure_centre_distance_minus_both_radii() {
        assert!((sphere_gap(Vec3::ZERO, 0.5, Vec3::new(3.0, 4.0, 0.0), 1.5) - 3.0).abs() < 1e-5);
        assert_eq!(sphere_gap(Vec3::ZERO, 2.0, Vec3::new(3.0, 0.0, 0.0), 2.0), 0.0);
    }

    #[test]
    fn a_sphere_next_to_a_large_box_measures_to_the_wall() {
        let (min, max) = (Vec3::new(2.0, -1.0, 0.0), Vec3::new(4.0, 1.0, 2.0));
        assert!((box_gap(Vec3::ZERO, 0.5, min, max) - 1.5).abs() < 1e-5);
        assert!((box_gap(Vec3::new(3.0, 0.0, 1.0), 0.5, min, max) - 0.0).abs() < 1e-5);
        assert_eq!(Shape::Box { min, max }.gap(Vec3::ZERO, 0.5), 1.5);
    }

    #[test]
    fn a_box_from_bevy_bounds_keeps_the_game_axes() {
        let shape = Shape::from_bevy_box(Vec3::new(-1.0, 0.0, -2.0), Vec3::new(3.0, 4.0, 2.0));
        let Shape::Box { min, max } = shape else { panic!("not a box") };
        // Bevy (x, y up, -z north) -> game (x east, y north, z up)
        assert_eq!((min, max), (Vec3::new(-1.0, -2.0, 0.0), Vec3::new(3.0, 2.0, 4.0)));
    }
}
