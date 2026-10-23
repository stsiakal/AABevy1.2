//! Doodad phases and growth.
//!
//! A doodad is drawn with the model of its current phase (`doodad_func_groups.model`, falling back to
//! `doodad_almighties.model`) and, when the phase grows (`DoodadFuncGrowth`), scaled from `start_scale` to `end_scale` as
//! the server's `time_left` runs out (the packet's `time_left` is the phase's exact remaining time).
//! `SCDoodadPhaseChanged` moves a drawn doodad to another phase: the
//! model under the root is replaced within a few frames (the root, its position and its base scale stay), and a phase
//! whose model does not change only gets its new scale. A doodad that had no model gets another try on a phase change.

use std::collections::VecDeque;

use aa_cry::scene::{CryAssets, ModelOptions, spawn_loaded};
use aa_cry::shade::illum::IllumMaterial;
use aa_net::state::{Doodads, Units};
use aa_world::coords::{cry_rot_to_bevy, cry_to_bevy};
use aa_net::{Event, NetEvent};
use bevy::prelude::*;

use super::data::{Db, DoodadGrowth};
use super::units::{RemoteDoodad, Views};

/// Model swaps per frame: each reads the pak and builds the model on the main thread.
const MAX_SWAPS: usize = 2;

/// What a drawn doodad shows now and how the current phase grows.
#[derive(Component)]
pub struct DoodadVisual {
    /// The pak path of the drawn model.
    pub path: String,
    /// The model entity under the doodad root.
    pub model: Option<Entity>,
    /// The phase the drawn model belongs to.
    pub func_group_id: u32,
    /// The phase's growth, `None` when it does not grow.
    pub growth: Option<DoodadGrowth>,
    /// Milliseconds of the growth left: the server's `time_left` at the spawn / phase change, counted down locally.
    pub remaining_ms: f32,
    /// The whole phase's duration (see [`DoodadGrowth::total_ms`]): the `time_left` announced by a phase change, or
    /// the longer of the database delay and the `time_left` at a spawn first seen mid-phase.
    pub total_ms: f32,
    /// The spawn packet's `doodad.scale` (the model's own size is multiplied by the growth).
    pub base_scale: f32,
}

impl DoodadVisual {
    /// The growth scale of the phase (`1` when it does not grow).
    pub fn growth_scale(&self) -> f32 {
        self.growth.map_or(1.0, |g| g.scale_at(self.remaining_ms, self.total_ms))
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<DoodadHighlights>().add_systems(
        Update,
        (phase_changes, cutdown, fall, grow, highlight).chain().after(super::units::sync_views).run_if(super::world_screens),
    )
    .add_systems(Update, follow_parents.before(super::units::sync_views).run_if(super::world_screens));
}

/// A doodad that hangs on a unit (a plot on its house, a deck item on a ship) is announced in the parent's frame: put
/// it at the parent's current place, so it is drawn, focused and reached where it stands, and moves with the parent.
pub fn follow_parents(
    units: Option<Res<aa_net::state::Units>>,
    doodads: Option<ResMut<Doodads>>,
    views: Option<Res<Views>>,
    db: Option<Res<Db>>,
    mut roots: Query<&mut Transform, With<RemoteDoodad>>,
) {
    let (Some(units), Some(mut doodads), Some(views)) = (units, doodads, views) else { return };
    if doodads.local.is_empty() {
        return;
    }
    let mut moved = doodads.bypass_change_detection().resolve_parents(&units);
    // a doodad on an attach point is placed again every frame: its entity may have been spawned after the pose settled
    let attached: Vec<u32> = doodads.local.keys().copied().filter(|id| doodads.by_id.get(id).is_some_and(|d| d.attach_point != 0) && !moved.contains(id)).collect();
    moved.extend(attached);
    if moved.is_empty() {
        return;
    }
    doodads.set_changed();
    for id in moved {
        let (Some(entity), Some(doodad)) = (views.doodad_entity(id), doodads.by_id.get(&id)) else { continue };
        let Ok(mut transform) = roots.get_mut(entity) else { continue };
        let [x, y, z, w] = doodad.rotation;
        let mut pos = Vec3::new(doodad.pos.x, doodad.pos.y, doodad.pos.z);
        // a doodad on an attach point of its slave (the Farm Wagon's cargo crates on `$cannon0..3`): the server announces it
        // at the slave's origin, the point's place in the body is rotated by the slave's heading
        if doodad.attach_point != 0
            && let (Some(db), Some(parent)) = (db.as_deref(), units.by_id.get(&doodad.parent_obj_id))
            && let Some(offset) = db.slave_attach_offset(parent.model_id, doodad.attach_point)
        {
            pos += attach_world_offset(offset, parent.rotation[2], parent.scale);
        }
        transform.translation = cry_to_bevy(pos);
        transform.rotation = cry_rot_to_bevy(Quat::from_xyzw(x, y, z, w).normalize());
    }
}

/// An attach point's body-frame `offset` as a world offset for a parent facing `yaw` (the rotation `resolve_parents` uses).
pub fn attach_world_offset(offset: Vec3, yaw: f32, scale: f32) -> Vec3 {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let (sin, cos) = yaw.sin_cos();
    Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z) * scale
}

/// The doodads that glow: the one the use bar is about and the one under the pointer (`hud::doodad` writes them, the
/// doodads whose `use_target_highlight` is off left out).
#[derive(Resource, Default)]
pub struct DoodadHighlights(pub Vec<u32>);

/// What a glowing doodad's materials were before: its model and each mesh's own material.
#[derive(Component)]
struct Glowing {
    model: Entity,
    originals: Vec<(Entity, GlowMaterial)>,
}

enum GlowMaterial {
    Standard(Handle<StandardMaterial>),
    Illum(Handle<IllumMaterial>),
}

/// The tint added to a glowing doodad's materials.
const GLOW: LinearRgba = LinearRgba::new(0.3, 0.26, 0.12, 0.0);

/// Lights the wanted doodads up (private copies of their materials with a warm emissive tint, so the doodads that
/// share the model keep theirs) and puts the others back.
#[allow(clippy::too_many_arguments)]
fn highlight(
    mut commands: Commands,
    wanted: Res<DoodadHighlights>,
    glowing: Query<(Entity, &Glowing, &RemoteDoodad, &DoodadVisual)>,
    plain: Query<(Entity, &RemoteDoodad, &DoodadVisual), Without<Glowing>>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut illum_meshes: Query<&mut MeshMaterial3d<IllumMaterial>>,
    mut illum_materials: ResMut<Assets<IllumMaterial>>,
) {
    for (entity, glow, remote, visual) in &glowing {
        if wanted.0.contains(&remote.obj_id) && visual.model == Some(glow.model) {
            continue;
        }
        for (mesh, original) in &glow.originals {
            match original {
                GlowMaterial::Standard(original) => {
                    if let Ok(mut m) = meshes.get_mut(*mesh) {
                        m.0 = original.clone();
                    }
                }
                GlowMaterial::Illum(original) => {
                    if let Ok(mut m) = illum_meshes.get_mut(*mesh) {
                        m.0 = original.clone();
                    }
                }
            }
        }
        commands.entity(entity).remove::<Glowing>();
    }
    for (entity, remote, visual) in &plain {
        if !wanted.0.contains(&remote.obj_id) {
            continue;
        }
        let Some(model) = visual.model else { continue };
        let mut originals = Vec::new();
        for mesh in children.iter_descendants(model) {
            if let Ok(mut m) = meshes.get_mut(mesh)
                && let Some(mut lit) = materials.get(&m.0).cloned()
            {
                lit.emissive += GLOW;
                originals.push((mesh, GlowMaterial::Standard(std::mem::replace(&mut m.0, materials.add(lit)))));
            } else if let Ok(mut m) = illum_meshes.get_mut(mesh)
                && let Some(mut lit) = illum_materials.get(&m.0).cloned()
            {
                // Basic Illum uses PBR emission; Full Illum reads its native emission uniform.
                lit.base.emissive += GLOW;
                lit.extension.params.emission += Vec4::new(GLOW.red, GLOW.green, GLOW.blue, 0.0);
                originals.push((mesh, GlowMaterial::Illum(std::mem::replace(&mut m.0, illum_materials.add(lit)))));
            }
        }
        commands.entity(entity).insert(Glowing { model, originals });
    }
}

/// `SCDoodadPhaseChanged`: the new phase's growth from the announced `time_left` at once, its model when it differs
/// a few per frame ([`MAX_SWAPS`]).
#[allow(clippy::too_many_arguments)]
pub fn phase_changes(
    mut events: MessageReader<NetEvent>,
    mut commands: Commands,
    db: Option<Res<Db>>,
    doodads: Res<Doodads>,
    mut views: ResMut<Views>,
    time: Res<Time>,
    mut roots: Query<(&RemoteDoodad, &mut DoodadVisual, &mut Transform)>,
    mut cry: CryAssets,
    mut swaps: Local<VecDeque<u32>>,
) {
    let Some(db) = db else { return };
    for NetEvent(event) in events.read() {
        match event {
            // a fresh state of a doodad that keeps its view: its `time_left` is announced again
            Event::DoodadsSpawned(spawned) => {
                for d in spawned {
                    views.doodad_announced(d.obj_id, time.elapsed_secs());
                }
            }
            Event::DoodadPhaseChanged { obj_id, func_group_id, time_left, .. } => {
                views.doodad_announced(*obj_id, time.elapsed_secs());
                let Some(entity) = views.doodad_entity(*obj_id) else {
                    // not drawn: if it never had a model (or is just out of range) the new phase may have one
                    views.retry_doodad(*obj_id);
                    continue;
                };
                let Ok((_, mut visual, mut transform)) = roots.get_mut(entity) else { continue };
                visual.func_group_id = *func_group_id;
                visual.growth = db.doodad_growth(*func_group_id);
                visual.remaining_ms = *time_left as f32;
                // the phase change announces the whole remaining time of the new phase: the growth ends with it
                visual.total_ms = visual.growth.map_or(0.0, |g| g.total_ms(visual.remaining_ms, false));
                // the new phase's size at once, in the frame the phase changed
                transform.scale = Vec3::splat(visual.base_scale * visual.growth_scale());
                if !swaps.contains(obj_id) {
                    swaps.push_back(*obj_id);
                }
            }
            _ => {}
        }
    }
    for _ in 0..MAX_SWAPS {
        let Some(obj_id) = swaps.pop_front() else { break };
        // gone, or drawn again from scratch (which already used the current phase)
        let Some(entity) = views.doodad_entity(obj_id) else { continue };
        let Ok((remote, mut visual, _)) = roots.get_mut(entity) else { continue };
        let Some(doodad) = doodads.by_id.get(&obj_id) else { continue };
        let func_group_id = visual.func_group_id;
        let Some(path) = db.doodad_phase_model(doodad.template_id, func_group_id).filter(|p| *p != visual.path) else { continue };
        match swap_model(&mut commands, &mut cry, entity, &path) {
            Some(model) => {
                // the old model goes only once the new one is there: no frame without a model
                if let Some(old) = visual.model.replace(model) {
                    commands.entity(old).despawn();
                }
                debug!("doodad {} phase {func_group_id}: model {path}", remote.obj_id);
                visual.path = path;
            }
            None => warn!("doodad {} phase {func_group_id}: cannot draw {path}", remote.obj_id),
        }
    }
}

/// A tree whose cutting down the server announced (`SCVegetationCutdowning`): it tips over, away from the cutter.
#[derive(Component)]
struct Falling {
    /// The rotation the tree stood with (Bevy space) and the horizontal axis it turns about (game space).
    base: Quat,
    axis: Vec3,
    seconds: f32,
}

/// How long the tree takes to fall, seconds.
const FALL_SECS: f32 = 2.2;

/// `SCVegetationCutdowning`: the doodad (in its cutting phase, still drawn as the whole tree) falls away from the cutter.
fn cutdown(
    mut events: MessageReader<NetEvent>,
    mut commands: Commands,
    views: Res<Views>,
    doodads: Res<Doodads>,
    units: Res<Units>,
    roots: Query<&Transform, (With<RemoteDoodad>, Without<Falling>)>,
) {
    for NetEvent(event) in events.read() {
        let Event::VegetationCutdown { unit_obj_id, doodad_obj_id } = event else { continue };
        let (Some(entity), Some(doodad)) = (views.doodad_entity(*doodad_obj_id), doodads.by_id.get(doodad_obj_id)) else { continue };
        let Ok(transform) = roots.get(entity) else { continue };
        // away from the cutter (a tree nobody stands near falls the way it faces)
        let away = units.get(*unit_obj_id).map(|u| Vec2::new(doodad.pos.x - u.pos.x, doodad.pos.y - u.pos.y)).filter(|d| d.length() > 0.1).unwrap_or(Vec2::Y).normalize();
        let axis = Vec3::Z.cross(away.extend(0.0)).normalize();
        commands.entity(entity).insert(Falling { base: transform.rotation, axis, seconds: 0.0 });
    }
}

/// Tips the falling trees over (slowly at first, then the weight takes them).
fn fall(time: Res<Time>, mut trees: Query<(&mut Falling, &mut Transform)>) {
    for (mut tree, mut transform) in &mut trees {
        if tree.seconds >= FALL_SECS {
            continue;
        }
        tree.seconds = (tree.seconds + time.delta_secs()).min(FALL_SECS);
        let angle = std::f32::consts::FRAC_PI_2 * (tree.seconds / FALL_SECS).powi(2);
        transform.rotation = cry_rot_to_bevy(Quat::from_axis_angle(tree.axis, angle)) * tree.base;
    }
}

/// Counts the growth down and scales the doodads that grow (packets announce `time_left`, they do not tick it).
pub fn grow(time: Res<Time>, mut roots: Query<(&mut DoodadVisual, &mut Transform)>) {
    let dt_ms = time.delta_secs() * 1000.0;
    for (mut visual, mut transform) in &mut roots {
        if visual.growth.is_none() {
            continue;
        }
        visual.remaining_ms = (visual.remaining_ms - dt_ms).max(0.0);
        let scale = visual.base_scale * visual.growth_scale();
        if (transform.scale.x - scale).abs() > 1e-4 {
            transform.scale = Vec3::splat(scale);
        }
    }
}

/// Loads `path` and puts its model under `root`; `None` (and the old model kept) when it cannot be loaded.
fn swap_model(commands: &mut Commands, cry: &mut CryAssets, root: Entity, path: &str) -> Option<Entity> {
    let options = ModelOptions::default();
    let loader = cry.loader();
    let prepared = loader.prepare_model(path, &options).ok()?;
    let loaded = cry.finish_model(prepared);
    let model = spawn_loaded(commands, path, &loaded, &options);
    commands.entity(model).insert(ChildOf(root));
    Some(model)
}

#[cfg(test)]
mod attach_tests {
    use super::*;

    #[test]
    fn an_attach_point_turns_with_the_parents_heading() {
        let offset = Vec3::new(-0.63, -1.94, 1.14);
        assert!(attach_world_offset(offset, 0.0, 1.0).abs_diff_eq(offset, 1e-6));
        // a quarter turn to the left: forward (+y) becomes -x, right (+x) becomes +y
        let turned = attach_world_offset(offset, std::f32::consts::FRAC_PI_2, 1.0);
        assert!(turned.abs_diff_eq(Vec3::new(1.94, -0.63, 1.14), 1e-5), "{turned:?}");
        assert!(attach_world_offset(offset, 0.0, 2.0).abs_diff_eq(offset * 2.0, 1e-6));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illum_highlight_uses_a_private_material_and_restores_the_original() {
        let mut app = App::new();
        app.init_resource::<DoodadHighlights>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<IllumMaterial>>()
            .add_systems(Update, highlight);
        let original = app.world_mut().resource_mut::<Assets<IllumMaterial>>().add(IllumMaterial::default());
        let root = app.world_mut().spawn(RemoteDoodad { obj_id: 7 }).id();
        let model = app.world_mut().spawn(ChildOf(root)).id();
        let mesh = app.world_mut().spawn((MeshMaterial3d(original.clone()), ChildOf(model))).id();
        let other = app.world_mut().spawn(MeshMaterial3d(original.clone())).id();
        app.world_mut().entity_mut(root).insert(DoodadVisual {
            path: String::new(), model: Some(model), func_group_id: 1, growth: None,
            remaining_ms: 0.0, total_ms: 0.0, base_scale: 1.0,
        });
        app.world_mut().resource_mut::<DoodadHighlights>().0.push(7);
        app.update();
        let highlighted = app.world().get::<MeshMaterial3d<IllumMaterial>>(mesh).unwrap().0.clone();
        assert_ne!(highlighted, original);
        assert_eq!(app.world().get::<MeshMaterial3d<IllumMaterial>>(other).unwrap().0, original);
        let materials = app.world().resource::<Assets<IllumMaterial>>();
        assert_eq!(materials.get(&original).unwrap().base.emissive, LinearRgba::BLACK);
        assert_eq!(materials.get(&highlighted).unwrap().base.emissive, LinearRgba::BLACK + GLOW);
        assert_eq!(materials.get(&highlighted).unwrap().extension.params.emission, Vec4::new(GLOW.red, GLOW.green, GLOW.blue, 0.0));
        app.update();
        assert_eq!(app.world().get::<MeshMaterial3d<IllumMaterial>>(mesh).unwrap().0, highlighted);
        app.world_mut().resource_mut::<DoodadHighlights>().0.clear();
        app.update();
        assert!(app.world().get::<Glowing>(root).is_none());
        assert_eq!(app.world().get::<MeshMaterial3d<IllumMaterial>>(mesh).unwrap().0, original);
    }
}
