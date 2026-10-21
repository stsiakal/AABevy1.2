//! What the server tells about a character's looks -> an aa-anim [`CharacterDesc`].
//!
//! The server sends race / gender, the 28 equipment slots (face, hair and the worn armour as item
//! template ids) and the creator's `ModelParams` (hair colour, skin colour, face sliders, decals,
//! colours). aa-anim wants the same things as database ids, which they are one to one.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use aa_anim::custom::Customization;
use aa_anim::db::{Decal, GameDb, ItemVisual};
use aa_anim::{Character, CharacterAssets, CharacterDesc, ClipLibrary, Costume, Gender, PartAsset, PartKind, Race};
use aa_net::model::{Equipment, Item, ItemDetail, ModelParams, race, slot};
use aa_net::state::Inventory;
use aa_net::{Event, NetEvent};
use aa_cry::scene::CryAssets;
use bevy::prelude::*;

use super::data::Db;
use super::player::LocalPlayer;
use super::units::RemoteUnit;
use super::weapons;

pub(crate) mod rebuild;

/// Armour slots in the order they are drawn (underwear first, outer layers last).
const WORN: [usize; 11] = [
    slot::UNDERPANTS,
    slot::UNDERSHIRT,
    slot::LEGS,
    slot::CHEST,
    slot::WAIST,
    slot::HANDS,
    slot::FEET,
    slot::ARMS,
    slot::NECK,
    slot::HEAD,
    slot::BACK,
];

/// Slots whose items can change the resolved look: the armour and body parts, the costume, the
/// carried weapons and the backpack. Earrings and rings are drawn by nothing here.
const LOOK_SLOTS: [usize; 24] = [
    slot::HEAD,
    slot::NECK,
    slot::CHEST,
    slot::WAIST,
    slot::LEGS,
    slot::HANDS,
    slot::FEET,
    slot::ARMS,
    slot::BACK,
    slot::UNDERSHIRT,
    slot::UNDERPANTS,
    slot::MAINHAND,
    slot::OFFHAND,
    slot::RANGED,
    slot::MUSICAL,
    slot::FACE,
    slot::HAIR,
    slot::GLASSES,
    slot::RESERVED,
    slot::TAIL,
    slot::BODY,
    slot::BEARD,
    slot::BACKPACK,
    slot::COSPLAY,
];

/// The equipment and model parameters a drawn character was described from. Received equipment
/// changes are folded into this source and the look is rebuilt from it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct LookSource {
    pub equipment: Equipment,
    pub params: ModelParams,
    /// An equipment change was received and the drawn parts have not been rebuilt from it yet.
    pub dirty: bool,
}

impl LookSource {
    pub fn new(equipment: &Equipment, params: &ModelParams) -> LookSource {
        LookSource { equipment: equipment.clone(), params: params.clone(), dirty: false }
    }

    /// Applies the slots of a received `SCUnitEquipmentsChanged`; true when a worn item changed.
    pub fn apply(&mut self, items: &[(u8, Option<Item>)]) -> bool {
        let mut changed = false;
        for (index, item) in items {
            let Some(slot) = self.equipment.slots.get_mut(usize::from(*index)) else { continue };
            if *slot != *item {
                *slot = item.clone();
                changed = true;
            }
        }
        changed
    }

    /// The same visual inputs (the dirty flag is request state, not appearance).
    pub fn same_inputs(&self, other: &LookSource) -> bool {
        self.equipment == other.equipment && self.params == other.params
    }
}

/// Whether a resolved look draws anything different (the resolved part list, plus the underwear
/// textures that replace the nude body's skin and have no part of their own).
pub fn parts_changed(old: &CharacterAssets, new: &CharacterAssets) -> bool {
    old.parts != new.parts || old.underwear != new.underwear
}

/// Take the client's authoritative worn equipment into `source`; true when a look slot changed.
/// The look uses the template and the image-item override of an item, not its instance details
/// (durability, gems), so those do not force a resolve.
pub fn sync_equipment(source: &mut LookSource, own: &Equipment) -> bool {
    let changed = LOOK_SLOTS.into_iter().any(|slot| !same_visual(source.equipment.get(slot), own.get(slot)));
    if changed {
        source.equipment = own.clone();
    }
    changed
}

fn same_visual(a: Option<&Item>, b: Option<&Item>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.template_id == b.template_id
                && match (&a.detail, &b.detail) {
                    (ItemDetail::Equip(a), ItemDetail::Equip(b)) => a.image_item_template_id == b.image_item_template_id,
                    (ItemDetail::Equip(a), _) => a.image_item_template_id == 0,
                    (_, ItemDetail::Equip(b)) => b.image_item_template_id == 0,
                    _ => true,
                }
        }
        _ => false,
    }
}

/// Race and gender of a unit: its model id (`models.id`, 10 / 11 nuian ... 20 / 21 ferre), else the
/// race / gender bytes of the packet.
pub fn race_gender(model_id: u32, race_id: u8, gender_id: u8) -> Option<(Race, Gender)> {
    Race::from_model_id(model_id).or_else(|| {
        let r = match race_id {
            race::NUIAN => Race::Nuian,
            race::ELF => Race::Elf,
            race::HARIHARAN => Race::Hariharan,
            race::FERRE => Race::Ferre,
            _ => return None,
        };
        Some((
            r,
            if gender_id == aa_net::model::gender::FEMALE {
                Gender::Female
            } else {
                Gender::Male
            },
        ))
    })
}

/// The look of a unit that is drawn like a player character.
pub fn describe(db: &GameDb, race: Race, gender: Gender, equipment: &Equipment, params: &ModelParams) -> Option<CharacterDesc> {
    let base = CharacterDesc::default_for(db, race, gender).ok()?;
    let mut desc = base.clone();
    let model = desc.model_id();

    // face: the customisable base face unless the item names another one
    if let Some(item) = equipment.get(slot::FACE).map(|i| i.template_id) {
        match db.body_part(item) {
            Ok(_) => desc.face = Some(item),
            Err(e) => {
                warn_missing(item, model, "face", &e.to_string());
                desc.face = None;
            }
        }
    }
    if let Some(item) = equipment.get(slot::HAIR).map(|i| i.template_id) {
        match db.body_part(item) {
            Ok(_) => desc.hair = Some(item),
            Err(e) => {
                warn_missing(item, model, "hair", &e.to_string());
                desc.hair = None;
            }
        }
    }
    if params.kind >= ModelParams::HAIR && params.hair_color != 0 && db.hair_color(params.hair_color).is_ok() {
        desc.hair_color = Some(params.hair_color);
    } else if desc.hair != base.hair {
        // a different hair style than the preset's: its own first colour
        desc.hair_color = None;
    }
    if let Some(item) = desc.hair.filter(|item| Some(*item) != base.hair && !hair_drawable(db, model, *item)) {
        // the server's database is newer than the 1.2 client: its npc-only / beauty-shop styles (nu_m_hair029 ...)
        // have meshes and materials here but no textures, and drew as flat grey sheets without the alpha cut
        debug!("appearance: hair style {item} has no textures in this client, the preset's style stands in");
        desc.hair_color = desc.hair_color.and_then(|id| same_colour(db, model, id, base.hair)).or(base.hair_color);
        desc.hair = base.hair;
    }
    if params.kind >= ModelParams::SKIN && params.skin_color != 0 {
        match db.skin_color(params.skin_color) {
            Ok(s) if s.model_id == model => desc.skin = params.skin_color,
            Ok(s) => warn!("skin colour {} belongs to model {}, not {model}", s.id, s.model_id),
            Err(e) => warn!("{e}"),
        }
    }
    if let Some(face) = params.face.as_ref().filter(|_| params.kind >= ModelParams::FACE) {
        let mut decals = Vec::new();
        if face.movable_decal_asset_id != 0 {
            decals.push(Decal {
                asset_id: face.movable_decal_asset_id,
                weight: face.movable_decal_weight,
                scale: face.movable_decal_scale,
                rotate: face.movable_decal_rotate,
                move_x: i32::from(face.movable_decal_move_x),
                move_y: i32::from(face.movable_decal_move_y),
            });
        }
        decals.extend(
            face.fixed_decals
                .iter()
                .filter(|(id, _)| *id != 0)
                .map(|&(asset_id, weight)| Decal {
                    asset_id,
                    weight,
                    ..Default::default()
                }),
        );
        desc.customization = Customization {
            modifier: face.modifier.clone(),
            normal_map: face.normal_map_id,
            normal_map_weight: face.normal_map_weight,
            lip_color: face.lip_color,
            left_pupil_color: face.left_pupil_color,
            right_pupil_color: face.right_pupil_color,
            eyebrow_color: face.eyebrow_color,
            deco_color: face.deco_color,
            decals,
        };
    }
    desc.costume = Costume::Items(worn_items(db, model, equipment));
    Some(desc)
}

/// Whether the hair style `item` can be drawn by this client: the colour maps of its material (that of its first
/// `hair_colors` row, else the mesh's own `.mtl`) are in the pak.
fn hair_drawable(db: &GameDb, model: u32, item: u32) -> bool {
    let Ok(part) = db.body_part(item) else { return false };
    let material = db
        .hair_colors_for(model, part.asset_ids[0])
        .ok()
        .and_then(|c| c.into_iter().next())
        .map(|c| c.material)
        .unwrap_or_else(|| part.path.rsplit_once('.').map_or(part.path.clone(), |(stem, _)| format!("{stem}.mtl")));
    let Ok(mtl) = aa_cry::material::Material::load(&material) else { return true };
    let pak = aa_pak::Pak::global();
    let subs = if mtl.sub_materials.is_empty() { std::slice::from_ref(&mtl) } else { &mtl.sub_materials[..] };
    subs.iter()
        .filter_map(|m| m.texture(&aa_cry::material::TextureMap::Diffuse))
        .all(|t| pak.exists(&t.pak_path()))
}

/// The `hair_colors` row of the hair style `hair` with the same colour (`comment`, `customNN`) as the row `id`.
fn same_colour(db: &GameDb, model: u32, id: u32, hair: Option<u32>) -> Option<u32> {
    let name = db.hair_color(id).ok()?.name;
    let asset = db.body_part(hair?).ok()?.asset_ids[0];
    db.hair_colors_for(model, asset).ok()?.into_iter().find(|c| c.name == name).map(|c| c.id)
}

/// Template ids of the worn armour that have meshes for this model (a look-change item stands in for
/// the item it covers).
fn worn_items(db: &GameDb, model: u32, equipment: &Equipment) -> Vec<u32> {
    WORN.into_iter()
        .chain([slot::COSPLAY])
        .filter_map(|s| {
            let item = equipment.get(s)?;
            selected_visuals(db, model, item, slot_name(s)).map(|(id, _)| id)
        })
        .collect()
}

/// The name of a worn slot for the one-time missing-piece warning.
fn slot_name(slot: usize) -> &'static str {
    match slot {
        slot::HEAD => "head",
        slot::NECK => "neck",
        slot::CHEST => "chest",
        slot::WAIST => "waist",
        slot::LEGS => "legs",
        slot::HANDS => "hands",
        slot::FEET => "feet",
        slot::ARMS => "arms",
        slot::BACK => "back",
        slot::UNDERSHIRT => "undershirt",
        slot::UNDERPANTS => "underpants",
        slot::COSPLAY => "cosplay",
        _ => "item",
    }
}

/// One `warn!` per (item id, model id): a rebuild repeats the same lookups, and a crowd wears the
/// same item many times over.
fn warn_missing(item: u32, model: u32, piece: &str, reason: &str) {
    static WARNED: OnceLock<Mutex<HashSet<(u32, u32)>>> = OnceLock::new();
    let mut warned = WARNED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if warned.insert((item, model)) {
        warn!("appearance: item {item} ({piece}) for model {model} is not drawn: {reason}");
    }
}

/// The exact visual already admitted by the costume assembler, including image-item fallback.
fn selected_visuals(db: &GameDb, model: u32, item: &Item, piece: &str) -> Option<(u32, Vec<ItemVisual>)> {
    let pak = aa_pak::Pak::global();
    let look = match &item.detail {
        ItemDetail::Equip(d) if d.image_item_template_id != 0 => d.image_item_template_id,
        _ => item.template_id,
    };
    let mut ids = vec![look];
    if item.template_id != look && item.template_id != 0 {
        ids.push(item.template_id);
    }
    let mut missing = Vec::new();
    for id in ids {
        match db.item_visuals(id, model) {
            Ok(visuals) if visuals.is_empty() => missing.push(format!("visual {id} has no item visuals for this model")),
            Ok(visuals) => match visuals.iter().map(|visual| &visual.path).find(|path| !pak.exists(path)) {
                Some(path) => missing.push(format!("visual {id}: the pak is missing {path}")),
                None => return Some((id, visuals)),
            },
            Err(e) => missing.push(format!("visual {id}: {e}")),
        }
    }
    warn_missing(item.template_id, model, piece, &missing.join("; "));
    None
}

/// Original39315820 uses the rendered cosplay descriptor before the head descriptor,
/// even when the former selects ordinary hair. Never infer flags from item names/classes.
fn headgear_detail(db: &GameDb, desc: &CharacterDesc, equipment: &Equipment) -> Option<u32> {
    let Costume::Items(rendered) = &desc.costume else { return None };
    for s in [slot::COSPLAY, slot::HEAD] {
        let Some((id, visuals)) = equipment.get(s).and_then(|item| selected_visuals(db, desc.model_id(), item, slot_name(s))) else {
            continue;
        };
        if !rendered.contains(&id) {
            continue;
        }
        let detail = visuals[0].detail;
        // All original armor/model groups have one shared detail. Refuse an
        // ambiguous replacement catalog rather than guessing which descriptor won.
        if visuals.iter().any(|visual| visual.detail != detail) {
            debug!("appearance: ambiguous headgear detail for item {id}, model {}", desc.model_id());
            return None;
        }
        return Some(detail);
    }
    None
}

/// Resolve received appearance with the hair variant of its actual rendered headgear.
/// Equipment visibility policy remains the costume assembler's responsibility.
pub fn resolve(db: &GameDb, desc: &CharacterDesc, equipment: &Equipment) -> aa_anim::Result<CharacterAssets> {
    let mut assets = match headgear_detail(db, desc, equipment) {
        Some(detail) => desc.resolve_with_headgear(db, Some(detail)),
        None => desc.resolve(db),
    }?;
    if let Some(proto) = assets.parts.first().cloned() {
        assets.parts.extend(extra_body_parts(db, desc.model_id(), equipment, &proto));
    }
    Ok(assets)
}

/// The body part slots `aa-anim` does not know: the ferre tail and the beard are skin attachments of their own
/// (`item_body_parts`, `.chr` on the base skeleton like the hair). Glasses (slot 21) have no data in the database.
/// `proto` is any part of the character (a body part has no equipment slot): the new parts copy what is not named here.
fn extra_body_parts(db: &GameDb, model: u32, equipment: &Equipment, proto: &PartAsset) -> Vec<PartAsset> {
    let pak = aa_pak::Pak::global();
    let mut out = Vec::new();
    for (s, name) in [(slot::TAIL, "tail"), (slot::BEARD, "beard")] {
        let Some(template) = equipment.get(s).map(|item| item.template_id).filter(|id| *id != 0) else { continue };
        let part = match db.body_part(template) {
            Ok(part) if part.model_id == model => part,
            Ok(part) => {
                warn_missing(template, model, name, &format!("its body part is for model {}", part.model_id));
                continue;
            }
            Err(e) => {
                warn_missing(template, model, name, &e.to_string());
                continue;
            }
        };
        if !pak.exists(&part.path) {
            warn_missing(template, model, name, &format!("the pak is missing {}", part.path));
            continue;
        }
        let (path, material, attachments) = if part.path.ends_with(".cdf") {
            match aa_anim::Cdf::load(&part.path) {
                Ok(cdf) => (cdf.model, cdf.material, cdf.attachments),
                Err(e) => {
                    warn_missing(template, model, name, &e.to_string());
                    continue;
                }
            }
        } else {
            (part.path, None, Vec::new())
        };
        out.push(PartAsset { kind: PartKind::Costume, slot: name.into(), path, material, dye: None, attachments, ..proto.clone() });
    }
    out
}

type LocalLooks<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static LocalPlayer,
        &'static mut LookSource,
        Option<&'static Character>,
        Option<&'static weapons::LookCarried>,
    ),
>;

/// Remote characters' `SCUnitEquipmentsChanged`: mark their look for the worker path of
/// [`super::units::sync_views`]. The local player learns its own changes from the item task
/// results in [`Inventory`] and is handled by [`sync_local_look`]. The persistent-id packet
/// (`EquipmentIdsChanged`) carries no visual change and is deliberately ignored.
pub fn watch_equipment_changes(
    mut events: MessageReader<NetEvent>,
    mut remotes: Query<(Entity, &RemoteUnit, &mut LookSource), Without<LocalPlayer>>,
) {
    for NetEvent(event) in events.read() {
        let Event::EquipmentChanged { obj_id, items } = event else { continue };
        for (_, unit, mut source) in &mut remotes {
            if unit.obj_id == *obj_id {
                if source.apply(items) {
                    source.dirty = true;
                }
                break;
            }
        }
    }
}

/// The local player's own equipment is authoritative in [`Inventory`]: `SCItemTaskSuccess` moves,
/// swaps and removals into/out of container 1 arrive there (the server does not broadcast
/// `SCUnitEquipmentsChanged` to the owner). Compare the look slots with [`LookSource`] and rebuild
/// only when the resolved parts or carried weapons changed. The initial equipment selected by
/// `SCCharacterState` is picked up the same way, and a stray own `SCUnitEquipmentsChanged` too
/// (aa-net folds it into the same inventory state).
pub fn sync_local_look(
    inventory: Option<Res<Inventory>>,
    db: Option<Res<Db>>,
    mut cry: CryAssets,
    clips: Res<ClipLibrary>,
    mut commands: Commands,
    mut locals: LocalLooks,
) {
    let Some(inventory) = inventory else { return };
    let Ok((entity, _, mut source, character, look_carried)) = locals.single_mut() else { return };
    if !sync_equipment(&mut source, &inventory.equipment) && !source.dirty {
        return;
    }
    source.dirty = true;
    let Some(character) = character else { return };
    let Some(db) = db.as_deref() else {
        warn!("appearance: no game database: the player's equipment change is not drawn");
        source.dirty = false;
        return;
    };
    let (race, gender) = (character.assets.desc.race, character.assets.desc.gender);
    let assets = {
        let game = db.game.lock().unwrap_or_else(|e| e.into_inner());
        match describe(&game, race, gender, &source.equipment, &source.params) {
            Some(desc) => match resolve(&game, &desc, &source.equipment) {
                Ok(assets) => Some(assets),
                Err(e) => {
                    warn!("appearance: cannot rebuild the player's look from its equipment: {e}");
                    None
                }
            },
            None => {
                warn!("appearance: cannot describe the player's look from its equipment");
                None
            }
        }
    };
    let Some(assets) = assets else {
        source.dirty = false;
        return;
    };
    let carried = weapons::carried(db, &source.equipment);
    let same_weapons = look_carried.is_some_and(|look| look.0 == carried);
    if !parts_changed(&character.assets, &assets) && same_weapons {
        source.dirty = false;
        return;
    }
    let rebuilt = rebuild::rebuild(&mut commands, &mut cry, &clips, entity, assets, carried, true);
    source.dirty = !rebuilt;
}

#[cfg(test)]
mod tests;
