//! Replaces a drawn character's parts in place when its resolved look changes.
//!
//! The spawn functions of aa-anim build a whole character hierarchy, including its bones. This
//! module spawns a replacement the same way (so the part assembly, facial morphs, `Cut` hulls and
//! materials are exactly the spawn path), then moves that hierarchy under the existing root and
//! rebinds the living [`Animator`] to the new bones. The root entity, its transform, movement,
//! controller and locomotion state stay; only the visual parts and the carried weapons change.

use aa_anim::facial::FacialAnimator;
use aa_anim::{Animator, Character, CharacterAssets, ClipLibrary, spawn_character, spawn_character_with_facial};
use aa_cry::scene::CryAssets;
use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;

use super::super::units::Asleep;
use super::super::weapons::{Carried, CarriedItem, LookCarried, Wield};

/// Builds `assets` as a temporary character and swaps its parts onto `target`. `facial` uses the
/// facial spawn like the local player; remote characters enable their face lazily after the swap.
/// Returns false (and leaves the old look) when the character cannot be spawned.
pub(crate) fn rebuild(
    commands: &mut Commands,
    cry: &mut CryAssets,
    clips: &ClipLibrary,
    target: Entity,
    assets: CharacterAssets,
    carried: Vec<CarriedItem>,
    facial: bool,
) -> bool {
    let spawned = if facial {
        spawn_character_with_facial(commands, cry, clips, assets, Transform::IDENTITY)
    } else {
        spawn_character(commands, cry, clips, assets, Transform::IDENTITY)
    };
    let temp = match spawned {
        Ok(entity) => entity,
        Err(e) => {
            warn!("appearance: cannot rebuild a character's look: {e}");
            return false;
        }
    };
    // never visible for a frame should the deferred swap be delayed
    commands.entity(temp).insert(Visibility::Hidden);
    commands.queue(move |world: &mut World| swap(world, target, temp, carried));
    true
}

fn swap(world: &mut World, target: Entity, temp: Entity, carried: Vec<CarriedItem>) {
    let old = world.get::<Character>(target).cloned();
    let Some(new) = world.entity_mut(temp).take::<Character>() else {
        warn!("appearance: a rebuilt character has no Character component");
        world.entity_mut(temp).despawn();
        return;
    };
    let new_bind = world.get::<Animator>(temp).map(|animator| (animator.rig.clone(), animator.bones.clone()));
    // the selected remote face owns private meshes of the old head; disable it before the parts go
    let had_face = world.get::<FacialAnimator>(target).is_some();
    if had_face {
        aa_anim::facial::disable_character_face(world, target);
    }
    let face = world.entity_mut(temp).take::<FacialAnimator>();
    let weights = world.entity_mut(temp).take::<MorphWeights>();

    world.entity_mut(new.model_root).insert(ChildOf(target));
    // remove first so the attachments system sees a fresh Character (Added) and spawns the new
    // CDF object attachments and bone effects
    world.entity_mut(target).remove::<Character>();
    world.entity_mut(target).insert(new);
    if let Some((rig, bones)) = new_bind {
        if let Some(mut animator) = world.get_mut::<Animator>(target) {
            animator.rig = rig;
            animator.bones = bones;
        } else if let Some(mut asleep) = world.get_mut::<Asleep>(target) {
            asleep.rebind(rig, bones);
        }
    }
    world.entity_mut(target).remove::<(FacialAnimator, MorphWeights)>();
    let mut inserted_face = false;
    if let (Some(face), Some(weights)) = (face, weights) {
        world.entity_mut(target).insert((face, weights));
        inserted_face = true;
    }
    // carried models hang on the old bones: they went with the old model root
    world.entity_mut(target).remove::<(Carried, Wield, LookCarried)>();
    if !carried.is_empty() {
        world.entity_mut(target).insert(Carried(carried.clone()));
    }
    world.entity_mut(target).insert(LookCarried(carried));
    if let Some(old) = old {
        world.entity_mut(old.model_root).despawn();
    }
    world.entity_mut(temp).despawn();
    if had_face && !inserted_face {
        // a sleeping unit keeps its Animator in Asleep; the facial setup needs it on the root
        let parked = world.entity_mut(target).take::<Asleep>();
        let was_parked = parked.is_some();
        if let Some(parked) = parked {
            world.entity_mut(target).insert(parked.into_animator());
        }
        if let Err(e) = aa_anim::facial::enable_character_face(world, target) {
            debug!("appearance: rebuilt character face unavailable: {e}");
        }
        if was_parked
            && let Some(animator) = world.entity_mut(target).take::<Animator>()
        {
            world.entity_mut(target).insert(Asleep::from_animator(animator));
        }
    }
}
