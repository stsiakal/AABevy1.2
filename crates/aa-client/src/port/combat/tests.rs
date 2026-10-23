//! Golden tests for the Stage A combat leaves.
//!
//! Each `goldens/*.json` file was recorded from the running 1.2 client by
//! `rust/tools/capture_combat_goldens.py` (lane `combat-golden`).  Integer results are compared
//! exactly.

use super::slot::slot_cooldowns;
use super::*;
use serde_json::Value;

enum Check {
    Pair((u32, u32)),
    Bool(bool),
    Scalar(u32),
}

fn load(name: &str) -> Value {
    let text = match name {
        "cooldown" => include_str!("goldens/cooldown.json"),
        "gcd" => include_str!("goldens/gcd.json"),
        "equality" => include_str!("goldens/equality.json"),
        "reaction" => include_str!("goldens/reaction.json"),
        "bighit" => include_str!("goldens/bighit.json"),
        "token" => include_str!("goldens/token.json"),
        "slot" => include_str!("goldens/slot.json"),
        other => panic!("unknown golden file {other}"),
    };
    serde_json::from_str(text).expect("golden JSON parses")
}

fn u32_of(value: &Value) -> u32 {
    value.as_u64().expect("unsigned") as u32
}

fn i32_of(value: &Value) -> i32 {
    value.as_i64().expect("signed") as i32
}

fn pair(value: &Value) -> (u32, u32) {
    let pair = value.as_array().expect("pair");
    (u32_of(&pair[0]), u32_of(&pair[1]))
}

fn entries(value: &Value) -> Vec<CooldownEntry> {
    value
        .as_array()
        .expect("entry list")
        .iter()
        .map(|row| {
            let row = row.as_array().expect("entry row");
            CooldownEntry {
                id: u32_of(&row[0]),
                remaining_ms: u32_of(&row[1]),
                total_ms: u32_of(&row[2]),
            }
        })
        .collect()
}

fn manager_from(value: &Value) -> CooldownManager {
    let (gcd_remaining_ms, gcd_total_ms) = pair(&value["gcd"]);
    let mut manager = CooldownManager {
        owner: u32_of(&value["owner"]),
        unit_id: u32_of(&value["unit_id"]),
        gcd_remaining_ms,
        gcd_total_ms,
        ..CooldownManager::default()
    };
    for entry in entries(&value["tags"]) {
        manager.tags[manager.tag_count as usize] = entry;
        manager.tag_count += 1;
    }
    for entry in entries(&value["skills"]) {
        manager.skills[manager.skill_count as usize] = entry;
        manager.skill_count += 1;
    }
    manager
}

fn desc_from(value: &Value) -> SkillDesc {
    SkillDesc {
        id: u32_of(&value["id"]),
        cooldown_tag_id: u32_of(&value["tag"]),
        ignore_global_cooldown: value["ignore"].as_bool().expect("ignore"),
        big_hit: i32_of(&value["big_hit"]),
        tags: value["tags"]
            .as_array()
            .expect("tags")
            .iter()
            .map(u32_of)
            .collect(),
    }
}

fn assert_manager(case: usize, name: &str, manager: &CooldownManager, out: &Value) {
    let want_gcd = pair(&out["gcd"]);
    assert_eq!(
        (manager.gcd_remaining_ms, manager.gcd_total_ms),
        want_gcd,
        "{name} case {case}: gcd"
    );
    let want_tags = entries(&out["tags"]);
    let got_tags = &manager.tags[..manager.tag_count.max(0) as usize];
    assert_eq!(got_tags, want_tags.as_slice(), "{name} case {case}: tags");
    let want_skills = entries(&out["skills"]);
    let got_skills = &manager.skills[..manager.skill_count.max(0) as usize];
    assert_eq!(
        got_skills,
        want_skills.as_slice(),
        "{name} case {case}: skills"
    );
}

#[test]
fn cooldown_matches_goldens() {
    let doc = load("cooldown");
    for (case_index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let init = &case["init"];
        let input = &case["input"];
        let mut manager = manager_from(init);
        let disabled = case["disabled"].as_bool().unwrap_or(false);
        let fn_name = case["fn"].as_str().expect("fn");
        let result = match fn_name {
            "cd_clear_global" => {
                manager.clear_global();
                None
            }
            "cd_reset" => {
                manager.reset(u32_of(&input["owner"]), u32_of(&input["unit_id"]));
                None
            }
            "cd_append" => {
                let skills = entries(&input["skills"]);
                let tags = entries(&input["tags"]);
                manager.append(&CooldownAppend {
                    skills: &skills,
                    tags: &tags,
                });
                None
            }
            "cd_tick" => {
                manager.tick(u32_of(&input["dt"]));
                None
            }
            "cd_clear_skill" => {
                manager.clear_skill(u32_of(&input["id"]));
                None
            }
            "cd_clear_tag" => {
                manager.clear_tag(u32_of(&input["id"]));
                None
            }
            "cd_insert" => {
                manager.insert(disabled, &desc_from(&input["desc"]), u32_of(&input["duration"]));
                None
            }
            "cd_query" => Some(Check::Pair(
                manager.query(&desc_from(&input["desc"]), input["ignore"].as_bool().unwrap()),
            )),
            "cd_is_on_cooldown" => Some(Check::Bool(manager.is_on_cooldown(
                &desc_from(&input["desc"]),
                input["ignore"].as_bool().unwrap(),
            ))),
            "cd_global_pair" => Some(Check::Pair(
                manager.global_pair(&desc_from(&input["desc"])),
            )),
            "cd_state" => Some(Check::Scalar(
                manager.state(&desc_from(&input["desc"])) as u32,
            )),
            "cd_remaining" => Some(Check::Scalar(
                manager.remaining(&desc_from(&input["desc"])),
            )),
            other => panic!("unknown cooldown fn {other}"),
        };
        assert_manager(case_index, fn_name, &manager, &case["out"]);
        if fn_name == "cd_reset" {
            assert_eq!(manager.owner, u32_of(&input["owner"]), "reset owner");
            assert_eq!(manager.unit_id, u32_of(&input["unit_id"]), "reset unit id");
        }
        match result {
            Some(Check::Pair(got)) => {
                let want = pair(&case["result"]);
                assert_eq!(got, want, "case {case_index} {fn_name}: query pair");
            }
            Some(Check::Bool(got)) => {
                let want = case["result"].as_bool().expect("bool");
                assert_eq!(got, want, "case {case_index} {fn_name}: bool");
            }
            Some(Check::Scalar(got)) => {
                assert_eq!(got, u32_of(&case["result"]), "case {case_index} {fn_name}: scalar");
            }
            None => {}
        }
    }
}

#[test]
fn gcd_matches_goldens() {
    let doc = load("gcd");
    for (case_index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let init = &case["init"];
        let input = &case["input"];
        let mut manager = manager_from(init);
        let disabled = case["disabled"].as_bool().unwrap_or(false);
        let attr = i32_of(&input["attr"]);
        match case["fn"].as_str().expect("fn") {
            "gcd_custom" => manager.set_custom_gcd(disabled, attr, i32_of(&input["custom"])),
            "gcd_default" => manager.set_default_gcd(disabled, attr),
            other => panic!("unknown gcd fn {other}"),
        }
        assert_eq!(
            (manager.gcd_remaining_ms, manager.gcd_total_ms),
            pair(&case["out"]["gcd"]),
            "gcd case {case_index}"
        );
    }
}

fn caster_from(value: &Value) -> SkillCaster {
    let words = value["words"].as_array().expect("words");
    let mut caster = SkillCaster {
        kind: u32_of(&value["kind"]),
        ..SkillCaster::default()
    };
    for (index, word) in words.iter().take(7).enumerate() {
        caster.words[index] = u32_of(word);
    }
    caster
}

fn target_from(value: &Value) -> SkillCastTarget {
    let words = value["words"].as_array().expect("words");
    let mut target = SkillCastTarget {
        kind: u32_of(&value["kind"]),
        ..SkillCastTarget::default()
    };
    for (index, word) in words.iter().take(12).enumerate() {
        target.words[index] = u32_of(word);
    }
    target
}

#[test]
fn equality_matches_goldens() {
    let doc = load("equality");
    for (case_index, case) in doc["caster_cases"].as_array().expect("caster").iter().enumerate() {
        let got = caster_eq(&caster_from(&case["a"]), &caster_from(&case["b"]));
        assert_eq!(
            got,
            case["out"].as_bool().expect("bool"),
            "caster case {case_index}"
        );
    }
    for (case_index, case) in doc["target_cases"].as_array().expect("target").iter().enumerate() {
        let got = target_eq(&target_from(&case["a"]), &target_from(&case["b"]));
        assert_eq!(
            got,
            case["out"].as_bool().expect("bool"),
            "target case {case_index}"
        );
    }
}

#[test]
fn reaction_matches_goldens() {
    let doc = load("reaction");
    for (weapon, key) in [(false, "normal_cases"), (true, "weapon_cases")] {
        for (case_index, case) in doc[key].as_array().expect(key).iter().enumerate() {
            let got = reaction(
                i32_of(&case["hit_type"]),
                weapon,
                case["critical"].as_bool().expect("critical"),
            );
            assert_eq!(
                (got.anim, got.kind),
                (u32_of(&case["out"][0]), u32_of(&case["out"][1])),
                "{key} case {case_index}"
            );
        }
    }
}

#[test]
fn big_hit_matches_goldens() {
    let doc = load("bighit");
    let resolved: Vec<u32> = doc["resolved_ids"]
        .as_array()
        .expect("resolved")
        .iter()
        .map(u32_of)
        .collect();
    let client_mode = i32_of(&doc["client_mode"]);
    for (case_index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let caster_id = u32_of(&case["caster_id"]);
        let record = HitRecordView {
            hit_type: i32_of(&case["hit_type"]),
            caster_kind: u32_of(&case["caster_kind"]),
            caster_id_20: caster_id,
            caster_id_2c: caster_id,
            target_id: u32_of(&case["target_id"]),
        };
        let got = big_hit(&record, i32_of(&case["big_hit"]), client_mode, |id| {
            resolved.contains(&id)
        });
        assert_eq!(
            got,
            case["out"].as_bool().expect("bool"),
            "big-hit case {case_index}"
        );
    }
}

#[test]
fn token_matches_goldens() {
    let doc = load("token");
    let local_id = u32_of(&doc["local"]["id"]);
    let local_ptr = u32_of(&doc["local"]["ptr"]);
    let npc_id = u32_of(&doc["npc"]["id"]);
    let npc_ptr = u32_of(&doc["npc"]["ptr"]);

    for (case_index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let state = &case["state"];
        let local = TokenUnit {
            id: local_id,
            ptr: local_ptr,
            target: u32_of(&state["local_target"]),
            watch: 0,
        };
        let npc = TokenUnit {
            id: npc_id,
            ptr: npc_ptr,
            target: u32_of(&state["npc_target"]),
            watch: 0,
        };
        let world = TokenWorld {
            local: Some(local),
            units: vec![local, npc],
            ..TokenWorld::default()
        };
        let text = case["token"].as_str().expect("token");
        let got = parse_token(&world, Some(text));
        let want_kind = u32_of(&case["out"]["kind"]);
        let want_ptr = u32_of(&case["out"]["ptr"]);
        assert_eq!(got.kind, want_kind, "token case {case_index} kind ({text:?})");
        if want_kind != TOKEN_KIND_NONE {
            assert_eq!(got.ptr, want_ptr, "token case {case_index} ptr ({text:?})");
        } else {
            assert_eq!(got.ptr, 0, "token case {case_index} failure pointer");
        }
    }
}

fn slot_seed(doc: &Value, seed: &str) -> CooldownManager {
    let mut manager = CooldownManager::default();
    let skill_desc = SkillDesc {
        id: u32_of(&doc["skill_id"]),
        cooldown_tag_id: u32_of(&doc["skill_desc_tag"]),
        ignore_global_cooldown: false,
        big_hit: 0,
        tags: doc["skill_desc_tags"]
            .as_array()
            .expect("desc tags")
            .iter()
            .map(u32_of)
            .collect(),
    };
    if seed.starts_with("skill") {
        manager.insert(false, &skill_desc, 1234);
    }
    let gcd = match seed {
        "gcd" | "item_gcd" => u32_of(&doc["gcd_default_ms"]),
        "gcd_custom" => u32_of(&doc["gcd_custom_ms"]),
        "skill_big_gcd" => 5000,
        _ => 0,
    };
    manager.gcd_remaining_ms = gcd;
    manager.gcd_total_ms = gcd;
    manager
}

#[test]
fn slot_matches_goldens() {
    let doc = load("slot");
    let skill_id = u32_of(&doc["skill_id"]);
    let item_id = u32_of(&doc["item_id"]);
    let item_skill_id = u32_of(&doc["item_skill_id"]);
    let item_kind = u32_of(&doc["item_kind"]);
    let skill_desc = SkillDesc {
        id: skill_id,
        cooldown_tag_id: u32_of(&doc["skill_desc_tag"]),
        ignore_global_cooldown: false,
        big_hit: 0,
        tags: doc["skill_desc_tags"]
            .as_array()
            .expect("desc tags")
            .iter()
            .map(u32_of)
            .collect(),
    };

    for (case_index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let seed = case["seed"].as_str().expect("seed");
        let manager = slot_seed(&doc, seed);
        let slot = ActionSlot {
            kind: u32_of(&case["input"]["kind"]),
            id: u32_of(&case["input"]["id"]),
        };
        let got = slot_cooldowns(
            &manager,
            slot,
            |id| (id == skill_id).then(|| skill_desc.clone()),
            |id| {
                (id == item_id).then_some(ItemAction {
                    skill_id: item_skill_id,
                    kind: item_kind,
                    cooldown: None,
                })
            },
            |id| id,
        );
        assert_eq!(
            got.global,
            pair(&case["out"]["global"]),
            "slot case {case_index} global"
        );
        assert_eq!(
            got.skill,
            pair(&case["out"]["skill"]),
            "slot case {case_index} skill"
        );
        assert_eq!(
            got.handled,
            matches!(slot.kind, 1 | 2) && (slot.kind != 1 || slot.id == item_id),
            "slot case {case_index} handled"
        );
    }
}

// ---------------------------------------------------------------- synthetic branches

#[test]
fn token_pet_team_login_preview_house_doodad_succeed() {
    let local = TokenUnit { id: 1, ptr: 0x100, target: 0, watch: 2 };
    let pet = TokenUnit { id: 3, ptr: 0x300, target: 1, ..TokenUnit::default() };
    let watched = TokenUnit { id: 2, ptr: 0x200, target: 0, ..TokenUnit::default() };
    let house = TokenUnit { id: 4, ptr: 0x400, target: 0, ..TokenUnit::default() };
    let team_unit = TokenUnit { id: 5, ptr: 0x500, target: 0, ..TokenUnit::default() };
    let login_unit = TokenUnit { id: 6, ptr: 0x600, target: 0, ..TokenUnit::default() };
    let preview_unit = TokenUnit { id: 7, ptr: 0x700, target: 0, ..TokenUnit::default() };
    let world = TokenWorld {
        local: Some(local),
        local_focus_item: Some(0xABC),
        pet: Some(pet),
        team: vec![
            Some(TeamMember { ptr: 0x900, unit: Some(team_unit) }),
            Some(TeamMember { ptr: 0x901, unit: None }),
        ],
        login: vec![None, Some(login_unit)],
        preview: vec![(3, 4, preview_unit)],
        house: Some(house.id),
        doodad: Some(0xDDD),
        units: vec![local, watched, pet, house, team_unit, login_unit, preview_unit],
    };
    assert_eq!(parse_token(&world, Some("playerpet")), TokenRef { kind: 1, ptr: 0x300 });
    assert_eq!(parse_token(&world, Some("playerpettarget")), TokenRef { kind: 1, ptr: 0x100 });
    assert_eq!(parse_token(&world, Some("watchtarget")), TokenRef { kind: 1, ptr: 0x200 });
    assert_eq!(parse_token(&world, Some("team1")), TokenRef { kind: 1, ptr: 0x500 });
    assert_eq!(parse_token(&world, Some("team2")), TokenRef { kind: 2, ptr: 0x901 });
    assert_eq!(parse_token(&world, Some("loginch1")), TokenRef { kind: 1, ptr: 0x600 });
    assert_eq!(parse_token(&world, Some("previewch3 4")), TokenRef { kind: 1, ptr: 0x700 });
    assert_eq!(parse_token(&world, Some("house")), TokenRef { kind: 1, ptr: 0x400 });
    assert_eq!(parse_token(&world, Some("doodad")), TokenRef { kind: 3, ptr: 0xDDD });
    assert_eq!(parse_token(&world, Some("doodadtarget")), TokenRef::none());
    assert_eq!(parse_token(&world, Some("targetfoo")), TokenRef { kind: 3, ptr: 0xABC });
    assert_eq!(parse_token(&world, Some("targettarget")), TokenRef::none());
    assert_eq!(parse_token(&world, None), TokenRef::none());
}

#[test]
fn slot_item_kind_nine_overrides_skill_pair() {
    let manager = CooldownManager {
        gcd_remaining_ms: 100,
        gcd_total_ms: 100,
        ..CooldownManager::default()
    };
    let desc = SkillDesc { id: 42, ..SkillDesc::default() };
    let got = get_action_cooldown_ex(
        &manager,
        ActionSlot { kind: 1, id: 7 },
        |_| Some(desc.clone()),
        |_| {
            Some(ItemAction {
                skill_id: 42,
                kind: 9,
                cooldown: Some((300, 2000)),
            })
        },
        |id| id,
    );
    assert_eq!(got.global, (100, 100));
    assert_eq!(got.skill, (300, 2000));
    assert!(got.handled);
    assert_eq!(
        get_action_cooldown(&manager, ActionSlot { kind: 0, id: 0 }, |_| None, |_| None, |id| id),
        (0, 0)
    );
}

#[test]
fn big_hit_threshold_is_zero() {
    let record = HitRecordView {
        hit_type: 1,
        caster_kind: 0,
        caster_id_20: 10,
        caster_id_2c: 0,
        target_id: 20,
    };
    assert!(big_hit(&record, 1, 1, |_| true));
    assert!(!big_hit(&record, 0, 1, |_| true));
    assert!(big_hit(&record, 0, 0, |_| true));
    assert!(big_hit(&record, 0, 1, |_| false));
}
