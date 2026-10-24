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
        "cast" => include_str!("goldens/cast.json"),
        "timeline" => include_str!("goldens/timeline.json"),
        "timeline_wide" => include_str!("goldens/timeline_wide.json"),
        "stop" => include_str!("goldens/stop.json"),
        "combo" => include_str!("goldens/combo.json"),
        "client_only" => include_str!("goldens/client_only.json"),
        "front" => include_str!("goldens/front.json"),
        "requirements" => include_str!("goldens/requirements.json"),
        "targeting_collect" => include_str!("goldens/targeting_collect.json"),
        "targeting_select" => include_str!("goldens/targeting_select.json"),
        "targeting_tab" => include_str!("goldens/targeting_tab.json"),
        "target_set" => include_str!("goldens/target_set.json"),
        "click_select" => include_str!("goldens/click_select.json"),
        "combat_state" => include_str!("goldens/combat_state.json"),
        "first_hit" => include_str!("goldens/first_hit.json"),
        "force_attack" => include_str!("goldens/force_attack.json"),
        "engage" => include_str!("goldens/engage.json"),
        "buff_append" => include_str!("goldens/buff_append.json"),
        other => panic!("unknown golden file {other}"),
    };
    serde_json::from_str(text).expect("golden JSON parses")
}

fn u32_of(value: &Value) -> u32 {
    value.as_u64().expect("unsigned") as u32
}

fn u32_or_zero(value: &Value) -> u32 {
    value.as_u64().unwrap_or(0) as u32
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


#[test]
fn cast_queries_match_goldens() {
    let doc = load("cast");
    let skill_id = u32_of(&doc["skill_id"]);
    let local_id = u32_of(&doc["local"]["id"]);

    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let slots = case["slots"].as_array().expect("slots");
        let key_of = |i: usize| u32_of(&slots[i]);
        let unit = CastUnit {
            cast_key: key_of(0),
            cast_delay_key: key_of(1),
            channel_key: key_of(2),
            channel_delay_key: key_of(3),
            ..CastUnit::default()
        };
        let state = SkillCastState {
            skill_id,
            normal_total: u32_or_zero(&case["normal_total"]),
            computed_delay: 0,
            channel_total: u32_or_zero(&case["channel_total"]),
            normal_start: u32_or_zero(&case["normal_start"]),
            channel_start: u32_or_zero(&case["channel_start"]),
        };
        let desc = CastDesc {
            name: doc["skill_name"].as_str().expect("name").to_string(),
            show_target_casting_time: case["desc_visible"].as_u64().unwrap_or(0) != 0,
            ..CastDesc::default()
        };
        let lookup_state = |key: u32| {
            (key != 0 && (key == unit.cast_key || key == unit.channel_key)).then_some(state)
        };
        let now = if case["label"] == "channel_mid" || case["label"] == "channel_clamped" {
            let total = state.channel_total;
            let current = case["current"].as_f64().unwrap_or(0.0) as u32;
            state.channel_start + (total - current)
        } else if state.normal_total != 0 {
            let current = case["current"].as_f64().unwrap_or(0.0) as u32;
            state.normal_start + current
        } else {
            0
        };
        let got = unit_casting_info(
            case["token"] != "bogus",
            &unit,
            now,
            |id| (id == skill_id).then(|| desc.clone()),
            lookup_state,
            |_| None,
        );
        if case["kind"] == "nil" {
            assert_eq!(got, CastingInfo::None, "cast case {index} {}", case["label"]);
        } else {
            let CastingInfo::Value { name, total, current, visible } = got else {
                panic!("cast case {index} {}: expected a value", case["label"]);
            };
            assert_eq!(name, doc["skill_name"].as_str().unwrap(), "cast case {index} name");
            assert_eq!(total, case["total"].as_f64().unwrap() as u32, "cast case {index} total");
            assert_eq!(
                current,
                case["current"].as_f64().unwrap() as u32,
                "cast case {index} current"
            );
            assert_eq!(visible, case["visible"].as_bool().unwrap(), "cast case {index} visible");
        }
    }

    for (index, case) in doc["can_cases"].as_array().expect("can").iter().enumerate() {
        let flag = u32_of(&case["flag"]);
        let desc = CastDesc {
            casting_cancelable: case["fn"] == "can_cast" && flag == 1,
            channeling_cancelable: case["fn"] == "can_channel" && flag == 1,
            controller_kinds: if case["label"] == "controller_22" { vec![0x22] } else { vec![] },
            ..CastDesc::default()
        };
        let unit = if case["fn"] == "can_cast" {
            CastUnit { cast_key: 1, ..CastUnit::default() }
        } else {
            CastUnit { channel_key: 1, ..CastUnit::default() }
        };
        let state = SkillCastState { skill_id, ..SkillCastState::default() };
        let got = if case["fn"] == "can_cast" {
            can_cast(&unit, |_| Some(desc.clone()), |key| (key == 1).then_some(state), |_| None)
        } else {
            can_channel(&unit, |_| Some(desc.clone()), |key| (key == 1).then_some(state), |_| None)
        };
        assert_eq!(got, case["out"].as_bool().unwrap(), "can case {index} {}", case["label"]);
    }

    for (index, case) in doc["combat_cases"].as_array().expect("combat").iter().enumerate() {
        let got = unit_combat_state(case["token"] == "player", u32_of(&case["flag"]) != 0);
        let want = case["out"]["value"].as_u64().unwrap() != 0;
        assert_eq!(got, want, "combat case {index}");
    }

    for (index, case) in doc["first_hit_cases"].as_array().expect("first").iter().enumerate() {
        let text = case["lua"].as_str().expect("lua");
        let want = text.ends_with(",true");
        let got = is_first_hit_by_me_or_my_team(
            true,
            u32_of(&case["hitter"]),
            u32_of(&case["team"]),
            local_id,
            0xFFFF,
            0,
        );
        assert_eq!(got, want, "first-hit case {index} {text}");
    }

    for (index, case) in doc["progress_cases"].as_array().expect("progress").iter().enumerate() {
        let kind = u32_of(&case["kind"]) as u8;
        let out = case["out"].as_u64().map(|v| v as u32);
        let (unit, state, now, live_key) = match case["label"].as_str().unwrap() {
            "cast" => {
                let total = 5000u32;
                let current = out.unwrap();
                (
                    CastUnit { cast_key: 1, ..CastUnit::default() },
                    Some(SkillCastState { skill_id, normal_total: total, ..SkillCastState::default() }),
                    total - current,
                    1,
                )
            }
            "channel" => {
                let total = 4000u32;
                let current = out.unwrap();
                (
                    CastUnit { channel_key: 2, ..CastUnit::default() },
                    Some(SkillCastState { skill_id, channel_total: total, ..SkillCastState::default() }),
                    total - current,
                    2,
                )
            }
            _ => (CastUnit::default(), None, 0, 0),
        };
        let got = cast_progress(
            &unit,
            now,
            kind,
            |key| state.filter(|_| key == live_key),
            |_| None,
            || out.unwrap_or(0),
        );
        if case["label"] == "none" || case["label"] == "unknown" {
            assert_eq!(got, None, "progress case {index} {}", case["label"]);
        } else {
            assert_eq!(got, out, "progress case {index} {}", case["label"]);
        }
    }
}

#[test]
fn cast_plot_states_and_first_hit_team() {
    let desc = CastDesc {
        name: "plot".into(),
        show_target_casting_time: false,
        ..CastDesc::default()
    };
    let unit = CastUnit { cast_delay_key: 7, ..CastUnit::default() };
    let plot = PlotCastState {
        plot_id: 5,
        cast_total: 900,
        cast_start: 100,
        channel_total: 400,
        channel_start: 200,
    };
    assert_eq!(
        unit_casting_info(
            true,
            &unit,
            600,
            |id| (id == 5).then(|| desc.clone()),
            |_| None,
            |key| (key == 7).then_some(plot)
        ),
        CastingInfo::Value { name: "plot".into(), total: 900, current: 500, visible: true }
    );
    let unit = CastUnit { channel_delay_key: 7, ..CastUnit::default() };
    assert_eq!(
        unit_casting_info(
            true,
            &unit,
            450,
            |id| (id == 5).then(|| desc.clone()),
            |_| None,
            |key| (key == 7).then_some(plot)
        ),
        CastingInfo::Value { name: "plot".into(), total: 400, current: 150, visible: true }
    );
    assert_eq!(
        cast_progress(&unit, 450, 7, |_| None, |key| (key == 7).then_some(plot), || 0),
        Some(150)
    );
    assert!(is_first_hit_by_me_or_my_team(true, 77, 4660, 77, 4660, 0));
    assert!(!is_first_hit_by_me_or_my_team(true, 77, 4661, 77, 4660, 0));
    assert!(is_first_hit_by_me_or_my_team(true, 0, 0, 77, 4660, 0));
    assert!(!is_first_hit_by_me_or_my_team(true, 78, 0, 0, 4660, 0));
    assert!(!is_first_hit_by_me_or_my_team(false, 77, 0, 77, 4660, 0));
}



fn timeline_manager(value: &Value) -> CooldownManager {
    let mut manager = CooldownManager::default();
    let gcd = value["gcd"].as_array().expect("gcd");
    manager.gcd_remaining_ms = u32_of(&gcd[0]);
    manager.gcd_total_ms = u32_of(&gcd[1]);
    for row in value["tags"].as_array().expect("tags") {
        manager.tags[manager.tag_count as usize] = CooldownEntry {
            id: u32_of(&row[0]),
            remaining_ms: u32_of(&row[1]),
            total_ms: u32_of(&row[2]),
        };
        manager.tag_count += 1;
    }
    for row in value["skills"].as_array().expect("skills") {
        manager.skills[manager.skill_count as usize] = CooldownEntry {
            id: u32_of(&row[0]),
            remaining_ms: u32_of(&row[1]),
            total_ms: u32_of(&row[2]),
        };
        manager.skill_count += 1;
    }
    manager
}

fn timeline_desc(id: u32, cast_time: i32, channel_time: i32) -> TimelineDesc {
    TimelineDesc {
        id,
        cast_time,
        channel_time,
        cooldown_ms: 0,
        cooldown_tag_id: 0,
        ignore_global_cooldown: true,
        custom_gcd_valid: false,
        custom_gcd: 0,
        target_type: 0,
        default_gcd: true,
        show_cast_bar: true,
        category_index: -1,
        category_id: 0,
        controllers: Vec::new(),
    }
}

fn timeline_world(local_id: u32, before: &Value) -> TimelineWorld {
    let mut world = TimelineWorld::default();
    world.descs.insert(10001, timeline_desc(10001, 0, 0));
    world.descs.insert(10024, timeline_desc(10024, 500, 0));
    world.descs.insert(10201, timeline_desc(10201, 0, 4000));
    world.descs.insert(2, timeline_desc(2, 0, 0));
    let unit = TimelineUnit {
        id: local_id,
        cast_key: u32_of(&before["slots"]["cast"]),
        cast_delay_key: u32_of(&before["slots"]["cast_delay"]),
        channel_key: u32_of(&before["slots"]["channel"]),
        channel_delay_key: u32_of(&before["slots"]["channel_delay"]),
        auto_attack: u32_of(&before["slots"]["auto_attack"]) != 0,
        cooldowns: timeline_manager(&before["cooldowns"]),
    };
    world.units.insert(local_id, unit);
    for (key, inst) in before["instances"].as_object().expect("instances") {
        let key: u32 = key.parse().expect("instance key");
        let skill_id = u32_of(&inst["skill_id"]);
        let timing = inst["timing"].as_array().expect("timing");
        world.instances.insert(
            key,
            SkillInstance {
                key,
                skill_id,
                caster: SkillCaster {
                    kind: 0,
                    pad: 0,
                    words: [local_id, 0, 0, 0, 0, 0, 0],
                },
                target: SkillCastTarget {
                    kind: 0,
                    pad: 0,
                    words: [key, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                },
                normal_total: u32_of(&timing[0]),
                computed_delay: u32_of(&timing[1]),
                channel_total: u32_of(&timing[2]),
                normal_start: u32_of(&timing[3]),
                channel_start: u32_of(&timing[4]),
            },
        );
    }
    world
}

fn local_caster(local_id: u32) -> SkillCaster {
    SkillCaster {
        kind: 0,
        pad: 0,
        words: [local_id, 0, 0, 0, 0, 0, 0],
    }
}

fn target_block(key: u32) -> SkillCastTarget {
    SkillCastTarget {
        kind: 0,
        pad: 0,
        words: [key, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    }
}

fn expected_events(label: &str) -> Vec<u32> {
    match label {
        "start_instant" | "start_cast" | "start_mirror" => vec![EVENT_SKILL_STARTED],
        "fire_cast" | "fire_channel" => vec![EVENT_SKILL_FIRED],
        "fire_without_start" => vec![EVENT_SKILL_STARTED, EVENT_SKILL_FIRED],
        "end_existing" | "end_missing" => vec![EVENT_SKILL_ENDED],
        "casting_stopped" => vec![EVENT_CASTING_STOPPED],
        _ => Vec::new(),
    }
}

#[test]
fn timeline_matches_goldens() {
    let doc = load("timeline");
    let local_id = u32_of(&doc["local"]["id"]);
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        if case["label"] == "client_only_fire" || case["label"] == "client_only_end" {
            continue;
        }
        let label = case["label"].as_str().expect("label");
        let before = &case["before"];
        let after = &case["after"];
        let mut world = timeline_world(local_id, before);
        let caster = local_caster(local_id);
        let key = after["instances"]
            .as_object()
            .and_then(|m| m.keys().next().cloned())
            .map(|k| k.parse::<u32>().unwrap())
            .or_else(|| before["instances"].as_object().and_then(|m| m.keys().next().cloned()).map(|k| k.parse().unwrap()))
            .unwrap_or(0x1234_5678);
        let timing = |v: &Value, i: usize| -> u32 {
            v["instances"]
                .as_object()
                .and_then(|m| m.values().next())
                .and_then(|inst| inst["timing"].as_array())
                .map(|t| u32_of(&t[i]))
                .unwrap_or(0)
        };
        match label {
            "start_instant" | "start_cast" => {
                let cast = if label == "start_instant" { 0 } else { 500 };
                world.now = timing(after, 3);
                let out = world.start_skill(
                    &caster,
                    &target_block(key),
                    if label == "start_instant" { 10001 } else { 10024 },
                    key,
                    cast,
                    false,
                    Some(1000),
                );
                assert_eq!(out, StartOutcome::Started, "timeline case {index} {label}");
                let inst = world.instances.get(&key).expect("instance");
                assert_eq!(inst.normal_total, cast);
                assert_eq!(inst.normal_start, timing(after, 3));
                assert_eq!(
                    world.units[&local_id].cast_key,
                    u32_of(&after["slots"]["cast"]),
                    "case {index} {label} cast slot"
                );
                assert_eq!(world.units[&local_id].cooldowns.gcd_total_ms, after["cooldowns"]["gcd"][1].as_u64().unwrap() as u32);
            }
            "start_mirror" => {
                let out = world.start_skill(&caster, &target_block(key), 10024, key, 500, true, Some(1000));
                assert_eq!(out, StartOutcome::Mirrored);
                assert!(world.instances.is_empty());
                assert_eq!(world.units[&local_id].cooldowns.gcd_total_ms, 0);
                assert_eq!(u32_of(&after["slots"]["cast"]), world.units[&local_id].cast_key);
            }
            "start_autoattack_no_target" => {
                let out = world.start_skill(&caster, &target_block(0), 2, 0, 0, false, None);
                assert_eq!(out, StartOutcome::AutoAttack);
                assert!(world.units[&local_id].auto_attack);
                assert_eq!(u32_of(&after["slots"]["auto_attack"]), 1);
            }
            "start_no_target_impossible" => {
                let out = world.start_skill(&caster, &target_block(0), 10024, 0, 500, false, None);
                assert_eq!(out, StartOutcome::ImpossibleNoTarget);
            }
            "start_invalid_skill" => {
                let out = world.start_skill(&caster, &target_block(key), 999999, key, 0, false, None);
                assert_eq!(out, StartOutcome::InvalidSkill);
            }
            "fire_cast" | "fire_channel" => {
                world.now = timing(after, 4);
                let (skill, time7, time9) = if label == "fire_cast" { (10024, 500, 0) } else { (10201, 0, 4000) };
                world.fire_skill(&caster, &target_block(key), skill, key, time7, time9, false, None);
                let got = world.instances.get(&key).expect("instance");
                assert_eq!(
                    (got.normal_total, got.computed_delay, got.channel_total, got.normal_start, got.channel_start),
                    (
                        timing(after, 0),
                        timing(after, 1),
                        timing(after, 2),
                        timing(after, 3),
                        timing(after, 4)
                    ),
                    "case {index} {label} timing"
                );
            }
            "fire_without_start" => {
                world.now = timing(after, 3);
                world.fire_skill(&caster, &target_block(key), 10024, key, 500, 0, false, Some(1000));
                let got = world.instances.get(&key).expect("instance");
                assert_eq!(
                    (got.normal_total, got.computed_delay, got.channel_total, got.normal_start, got.channel_start),
                    (
                        timing(after, 0),
                        timing(after, 1),
                        timing(after, 2),
                        timing(after, 3),
                        timing(after, 4)
                    ),
                    "case {index} {label} timing"
                );
                assert_eq!(world.units[&local_id].cooldowns.gcd_total_ms, 1000);
            }
            "end_existing" | "end_missing" => {
                let key = before["instances"]
                    .as_object()
                    .and_then(|m| m.keys().next().cloned())
                    .map(|k| k.parse::<u32>().unwrap())
                    .unwrap_or(0x1234_5678);
                world.end_skill(key);
                assert!(world.instances.is_empty());
                assert_eq!(u32_of(&after["slots"]["cast"]), world.units[&local_id].cast_key);
            }
            "stop_other" => {
                world.stop_skill(local_id, 10024);
                assert!(!world.units[&local_id].auto_attack);
            }
            "casting_stopped" => {
                let key = local_id;
                world.casting_stopped(key, 777);
                let row = world.units[&local_id].cooldowns.skills[0];
                assert_eq!((row.id, row.total_ms), (10024, 777));
                assert!(world.instances.contains_key(&key));
                assert_eq!(u32_of(&after["slots"]["cast"]), world.units[&local_id].cast_key);
            }
            "casting_delayed" => {
                let key = before["instances"]
                    .as_object()
                    .and_then(|m| m.keys().next().cloned())
                    .map(|k| k.parse::<u32>().unwrap())
                    .expect("key");
                world.instances.get_mut(&key).unwrap().normal_start = timing(after, 3);
                world.now = timing(after, 3) + 100;
                world.casting_delayed(key, 250);
                assert_eq!(world.instances[&key].normal_total, timing(after, 0));
            }
            "stop_casting" => {
                let key = before["instances"]
                    .as_object()
                    .and_then(|m| m.keys().next().cloned())
                    .map(|k| k.parse::<u32>().unwrap())
                    .expect("key");
                assert!(world.stop_casting(key));
                assert_eq!(u32_of(&after["slots"]["cast"]), world.units[&local_id].cast_key);
            }
            other => panic!("unknown timeline case {other}"),
        }
        assert_eq!(world.events, expected_events(label), "case {index} {label} events");
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

// ------------------------------------------------------------ stage 3 live goldens

fn live_desc(id: u32) -> Option<TimelineDesc> {
    let (cast, channel, cooldown) = match id {
        10001 => (0, 0, 30000),
        10024 => (500, 0, 60000),
        10201 => (0, 4000, 18000),
        10372 => (0, 12000, 120000),
        10501 => (0, 0, 30000),
        2 => (0, 0, 300),
        _ => return None,
    };
    Some(TimelineDesc {
        id,
        cast_time: cast,
        channel_time: channel,
        cooldown_ms: cooldown,
        cooldown_tag_id: 0,
        ignore_global_cooldown: false,
        custom_gcd_valid: false,
        custom_gcd: 0,
        target_type: 0,
        default_gcd: true,
        show_cast_bar: true,
        category_index: -1,
        category_id: 0,
        controllers: Vec::new(),
    })
}

fn live_world(before: &Value, now: u32) -> TimelineWorld {
    let mut world = TimelineWorld { now, ..TimelineWorld::default() };
    for id in [2, 10001, 10024, 10201, 10372, 10501] {
        world.descs.insert(id, live_desc(id).unwrap());
    }
    let local_id = before["local_id"].as_u64().unwrap_or(0) as u32;
    let unit = TimelineUnit {
        id: local_id,
        cast_key: u32_of(&before["slots"]["cast"]),
        cast_delay_key: u32_of(&before["slots"]["cast_delay"]),
        channel_key: u32_of(&before["slots"]["channel"]),
        channel_delay_key: u32_of(&before["slots"]["channel_delay"]),
        auto_attack: u32_of(&before["slots"]["auto_attack"]) != 0,
        cooldowns: timeline_manager(&before["cooldowns"]),
    };
    world.units.insert(local_id, unit);
    for (key, inst) in before["instances"].as_object().expect("instances") {
        let key: u32 = key.parse().expect("key");
        let timing = inst["timing"].as_array().expect("timing");
        world.instances.insert(
            key,
            SkillInstance {
                key,
                skill_id: u32_of(&inst["skill_id"]),
                caster: local_caster(local_id),
                target: target_block(key),
                normal_total: u32_of(&timing[0]),
                computed_delay: u32_of(&timing[1]),
                channel_total: u32_of(&timing[2]),
                normal_start: u32_of(&timing[3]),
                channel_start: u32_of(&timing[4]),
            },
        );
    }
    world
}

fn golden_before(case: &Value, local_id: u32) -> Value {
    let mut before = case["before"].clone();
    before["local_id"] = Value::from(local_id);
    before
}

fn timing_of(value: &Value, index: usize) -> u32 {
    value["instances"]
        .as_object()
        .and_then(|m| m.values().next())
        .and_then(|inst| inst["timing"].as_array())
        .map(|t| u32_of(&t[index]))
        .unwrap_or(0)
}

fn assert_instances(case: &Value, world: &TimelineWorld, label: &str) {
    let after = &case["after"]["instances"];
    let want = after.as_object().expect("instances");
    assert_eq!(world.instances.len(), want.len(), "{label}: instance count");
    for (key, inst) in want {
        let key: u32 = key.parse().expect("key");
        let got = world.instances.get(&key).unwrap_or_else(|| panic!("{label}: missing {key}"));
        let timing = inst["timing"].as_array().expect("timing");
        assert_eq!(got.skill_id, u32_of(&inst["skill_id"]), "{label}: skill id");
        assert_eq!(got.normal_total, u32_of(&timing[0]), "{label}: normal total");
        assert_eq!(got.normal_start, u32_of(&timing[3]), "{label}: normal start");
    }
}

#[test]
fn wide_timeline_matches_live_goldens() {
    let doc = load("timeline_wide");
    let local_id = u32_of(&doc["local"]["id"]);
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let label = case["label"].as_str().expect("label");
        let mut before = golden_before(case, local_id);
        if label.starts_with("stop_casting") || label.starts_with("casting_stopped")
            || label.starts_with("casting_delayed")
        {
            before["slots"]["cast"] = case["before"]["slots"]["cast"].clone();
        }
        let key = case["before"]["instances"]
            .as_object()
            .and_then(|m| m.keys().next().cloned())
            .map(|k| k.parse::<u32>().unwrap())
            .or_else(|| {
                case["after"]["instances"]
                    .as_object()
                    .and_then(|m| m.keys().next().cloned())
                    .map(|k| k.parse().unwrap())
            })
            .unwrap_or(0x7f0000 + index as u32);
        let mut world = live_world(&before, timing_of(&case["after"], 4).max(timing_of(&case["after"], 3)));
        let caster = local_caster(local_id);
        let target = target_block(key);
        if label.starts_with("start_wide") {
            let cast = u32_of(&case["cast_time"]);
            let mirror = case["mirror"].as_bool().unwrap();
            let target_key = u32_of(&case["target_key"]);
            let gcd = {
                let before_gcd = u32_of(&before["cooldowns"]["gcd"][1]);
                let after_gcd = u32_of(&case["after"]["cooldowns"]["gcd"][1]);
                (after_gcd != before_gcd).then_some(after_gcd)
            };
            world.now = timing_of(&case["after"], 3);
            let out = world.start_skill(
                &caster, &target, u32_of(&case["skill_id"]), target_key, cast, mirror, gcd,
            );
            if u32_of(&case["skill_id"]) == 999999 {
                assert_eq!(out, StartOutcome::InvalidSkill, "{label}");
            } else if mirror {
                assert_eq!(out, StartOutcome::Mirrored, "{label}");
            } else if target_key == 0 {
                if u32_of(&case["skill_id"]) == AUTO_ATTACK_SKILL {
                    assert_eq!(out, StartOutcome::AutoAttack, "{label}");
                } else {
                    assert_eq!(out, StartOutcome::ImpossibleNoTarget, "{label}");
                }
            } else {
                assert_eq!(out, StartOutcome::Started, "{label}");
            }
            assert_instances(case, &world, label);
            assert_eq!(
                world.units[&local_id].cast_key,
                u32_of(&case["after"]["slots"]["cast"]),
                "{label}: cast slot"
            );
            if !mirror {
                assert_eq!(
                    world.units[&local_id].cooldowns.gcd_total_ms,
                    u32_of(&case["after"]["cooldowns"]["gcd"][1]),
                    "{label}: gcd"
                );
            }
        } else if label.starts_with("fire_wide") {
            let before_key = case["before"]["instances"]
                .as_object()
                .and_then(|m| m.keys().next().cloned())
                .map(|k| k.parse::<u32>().unwrap());
            let Some(before_key) = before_key else { continue };
            world.fire_skill(
                &caster,
                &target_block(before_key),
                u32_of(&case["skill_id"]),
                before_key,
                u32_of(&case["time7"]),
                u32_of(&case["time9"]),
                case["error"].as_bool().unwrap(),
                None,
            );
            assert_instances(case, &world, label);
        } else if label.starts_with("end_wide") {
            world.end_skill(u32_of(&case["target_key"]));
            assert_instances(case, &world, label);
        } else if label.starts_with("casting_stopped") {
            world.casting_stopped(key, u32_of(&case["duration"]));
            assert_instances(case, &world, label);
            assert_eq!(
                world.units[&local_id].cast_key,
                u32_of(&case["after"]["slots"]["cast"]),
                "{label}: cast slot"
            );
        } else if label.starts_with("casting_delayed") {
            let before_total = timing_of(&case["before"], 0);
            let after_total = timing_of(&case["after"], 0);
            world.now = timing_of(&case["after"], 3).max(1) - 1;
            world.casting_delayed(key, u32_of(&case["duration"]));
            if after_total != before_total {
                assert_eq!(
                    world.instances[&key].normal_total, after_total,
                    "{label}: delayed total"
                );
            }
        } else if label.starts_with("stop_casting") {
            let got = world.stop_casting(u32_of(&case["target_key"]));
            let want = (u32_of(&case["eax"]) & 1) != 0;
            assert_eq!(got, want, "{label}: stop_casting return");
        }
    }
}

#[test]
fn stop_matches_live_goldens() {
    let doc = load("stop");
    for case in doc["cases"].as_array().expect("cases").iter() {
        let label = case["label"].as_str().unwrap();
        let unit_id = u32_of(&case["unit_id"]);
        let local_id = if unit_id == 0 || unit_id == 0xffff_fffe { 7 } else { unit_id };
        let mut before = case["before"].clone();
        before["local_id"] = Value::from(local_id);
        let mut world = live_world(&before, 0);
        world.stop_skill(unit_id, u32_of(&case["skill_id"]));
        let armed = u32_of(&case["after"]["slots"]["auto_attack"]) != 0;
        assert_eq!(
            world.units[&local_id].auto_attack, armed, "{label}: local auto-attack flag"
        );
        if case["events"].as_array().unwrap().contains(&Value::from(23)) {
            assert_eq!(world.events, vec![EVENT_AUTO_ATTACK_STOPPED], "{label}: event");
            let row = world.units[&local_id].cooldowns.skills[0];
            assert_eq!(row.id, 2, "{label}: auto cooldown id");
            assert_eq!(row.total_ms, 300, "{label}: auto cooldown total");
        } else {
            assert!(world.events.is_empty(), "{label}: no event");
        }
    }
}

#[test]
fn combo_add_matches_live_goldens() {
    let doc = load("combo");
    let local_id = 1;
    for case in doc["cases"].as_array().expect("cases").iter() {
        if case.get("skill_id").is_none() {
            continue;
        }
        let label = case["label"].as_str().unwrap();
        let skill = u32_of(&case["skill_id"]);
        let next = u32_of(&case["next_skill"]);
        let margin = u32_of(&case["margin"]);
        let duration = u32_of(&case["duration"]);
        let start = case["after"]["combo"]
            .as_array()
            .and_then(|rows| rows.first())
            .map(|row| u32_of(&row[3]))
            .unwrap_or(0);
        let mut world = TimelineWorld { now: start, ..TimelineWorld::default() };
        for id in [10001, 10024, 10201, 10501, 2] {
            if let Some(desc) = live_desc(id) {
                world.descs.insert(id, desc);
            }
        }
        if next != 999999 {
            world.descs.insert(next, live_desc(next).unwrap());
        }
        world.units.insert(local_id, TimelineUnit { id: local_id, ..TimelineUnit::default() });
        let desc = TimelineDesc {
            controllers: vec![TimelineController {
                kind: 0x18,
                values: vec![0x30, next as i32, margin as i32],
            }],
            ..live_desc(skill).unwrap()
        };
        let added = world.combo_add(local_id, &desc, duration);
        let want = case["after"]["combo"].as_array().unwrap();
        if want.is_empty() {
            assert!(!added, "{label}: no entry");
            assert!(world.combo.is_empty(), "{label}: combo vector");
        } else {
            assert!(added, "{label}: added");
            assert_eq!(world.combo.len(), 1, "{label}: one entry");
            let got = &world.combo[0];
            assert_eq!(got.key, u32_of(&want[0][0]), "{label}: key");
            assert_eq!(got.current_skill_id, u32_of(&want[0][1]), "{label}: current");
            assert_eq!(got.next_skill_id, u32_of(&want[0][2]), "{label}: next");
            assert_eq!(got.start, u32_of(&want[0][3]), "{label}: start");
            assert_eq!(got.duration, u32_of(&want[0][4]), "{label}: duration");
            assert_eq!(got.margin, u32_of(&want[0][5]), "{label}: margin");
            if duration != 0 {
                let row = world.units[&local_id].cooldowns.skills[0];
                assert_eq!(row.id, next, "{label}: cooldown id");
                assert_eq!(row.total_ms, duration, "{label}: cooldown total");
            }
        }
    }
}

#[test]
fn combo_update_matches_live_goldens() {
    let doc = load("combo");
    let local_id = 1;
    for case in doc["update_cases"].as_array().expect("update").iter() {
        let label = case["label"].as_str().unwrap();
        let entries = case["entries"].as_array().unwrap();
        let mut world = TimelineWorld::default();
        world.descs.insert(10201, live_desc(10201).unwrap());
        world.units.insert(local_id, TimelineUnit { id: local_id, ..TimelineUnit::default() });
        for row in entries {
            let row = row.as_array().unwrap();
            world.combo.push(ComboEntry {
                key: u32_of(&row[0]),
                current_skill_id: u32_of(&row[1]),
                next_skill_id: u32_of(&row[2]),
                start: u32_of(&row[3]),
                duration: u32_of(&row[4]),
                margin: u32_of(&row[5]),
            });
        }
        let now = entries
            .iter()
            .map(|row| {
                let row = row.as_array().unwrap();
                u32_of(&row[3])
                    .wrapping_add(u32_of(&row[4]))
                    .wrapping_add(u32_of(&row[5]))
            })
            .max()
            .unwrap_or(0)
            .wrapping_add(1);
        world.now = now;
        world.combo_update();
        assert!(world.combo.is_empty(), "{label}: combo cleared");
        let row = world.units[&local_id].cooldowns.skills[0];
        assert_eq!(row.id, 10201, "{label}: cooldown id");
        assert_eq!(
            row.total_ms,
            u32_of(&case["next_cooldown_ms"]),
            "{label}: skill cooldown"
        );
    }
}

#[test]
fn client_only_matches_live_goldens() {
    let doc = load("client_only");
    let local_id = 1;
    for case in doc["cases"].as_array().expect("cases").iter() {
        let label = case["label"].as_str().unwrap();
        let input = &case["input"];
        let mut world = TimelineWorld::default();
        world.descs.insert(10001, live_desc(10001).unwrap());
        world.descs.insert(10024, live_desc(10024).unwrap());
        world.units.insert(local_id, TimelineUnit { id: local_id, ..TimelineUnit::default() });
        world.client_only.push(ClientSkill {
            skill_id: u32_of(&input["skill_id"]),
            target_key: u32_of(&input["target_key"]),
            normal_total: 500,
            computed_delay: 100,
            fire_time: u32_of(&input["fire_time"]),
            end_time: u32_of(&input["end_time"]),
            fired: u32_of(&input["fired"]) != 0,
            damaged: u32_of(&input["damaged"]) != 0,
            local_damage: u32_of(&input["local"]) != 0,
            has_hit_record: input["has_hit"].as_bool().unwrap_or(false),
            ..ClientSkill::default()
        });
        let end_time = u32_of(&input["end_time"]);
        let fire_time = u32_of(&input["fire_time"]);
        let got = &case["client"];
        let damaged_after = u32_of(&got["damaged"]) != 0;
        let removed = case["after"]["client_only"].as_array().unwrap().is_empty();
        world.now = if u32_of(&input["fired"]) == 0 {
            fire_time.wrapping_add(1)
        } else if removed {
            end_time.wrapping_add(1001)
        } else if damaged_after {
            end_time.wrapping_add(1)
        } else {
            end_time.wrapping_sub(1)
        };
        let mut calls = Vec::new();
        world.update_client_only_with(|_, call| calls.push(call.clone()));
        let skill = &world.client_only;
        if skill.is_empty() {
            assert_eq!(u32_of(&got["fired"]), 1, "{label}: fired");
        } else {
            assert_eq!(skill[0].fired, got["fired"].as_u64().unwrap() != 0, "{label}: fired");
            assert_eq!(
                skill[0].damaged,
                got["damaged"].as_u64().unwrap() != 0,
                "{label}: damaged"
            );
        }
        let fire_args = case["fire_args"].as_array().unwrap();
        if u32_of(&fire_args[0]) != 0 {
            assert_eq!(calls.len(), 1, "{label}: fire call");
            assert_eq!(calls[0].skill_id, u32_of(&fire_args[0]), "{label}: fire skill");
            assert_eq!(calls[0].target_key, u32_of(&fire_args[1]), "{label}: fire target");
            assert_eq!(calls[0].time7, u32_of(&fire_args[5]), "{label}: fire time7");
        } else {
            assert!(calls.is_empty(), "{label}: no fire call");
        }
        if case["events"].as_array().unwrap().contains(&Value::from(22)) {
            assert!(world.events.contains(&EVENT_SKILL_ENDED), "{label}: end event");
        }
    }
}

#[test]
fn front_matches_live_goldens() {
    let doc = load("front");
    for case in doc["cases"].as_array().expect("cases").iter() {
        let label = case["label"].as_str().unwrap();
        let skill = u32_of(&case["skill_id"]);
        let target = u32_of(&case["target_id"]);
        let refusal = case["refusal"].as_array().unwrap();
        let packet = &case["packet"];
        let body = packet["body"].as_str();
        let outcome = use_skill(
            &UseSkillInput {
                unit_id: 41779,
                unit_present: true,
                skill_id: skill,
                desc_present: skill != 999999,
                desc_target_type: 0,
                caster_id: 41779,
                target_id: target,
                caster_moving: false,
                auto_attack_skill: 2,
                gate: (skill == 2).then_some(GateInput {
                    local_present: true,
                    target_present: target != 0,
                    target_attached: false,
                    owner_type: 0,
                    relation: 0,
                    relation_result: (0x2f, 0x65),
                }),
            },
            |_req| {
                if body.is_some() {
                    (0, 0)
                } else if refusal.len() >= 2 {
                    (u32_of(&refusal[0]) as u8, (u32_of(&refusal[1]) & 0xffff) as u16)
                } else {
                    (0, 0)
                }
            },
        );
        if skill == 999999 {
            assert_eq!(outcome, UseSkillOutcome::Invalid, "{label}");
            continue;
        }
        match outcome {
            UseSkillOutcome::Sent(req) => {
                let body = body.unwrap_or_else(|| panic!("{label}: expected a packet"));
                let bytes: Vec<u8> = body
                    .split_whitespace()
                    .map(|b| u8::from_str_radix(b, 16).unwrap())
                    .collect();
                let payload = &bytes[6..];
                assert_eq!(req.body(), payload, "{label}: request bytes");
            }
            UseSkillOutcome::Refused(got) => {
                assert!(body.is_none(), "{label}: no packet on refusal");
                assert_eq!(got.result, u32_of(&refusal[0]) as u8, "{label}: result");
                if refusal.len() >= 2 {
                    assert_eq!(got.arg, (u32_of(&refusal[1]) & 0xffff) as u16, "{label}: arg");
                }
            }
            UseSkillOutcome::Invalid => panic!("{label}: unexpected invalid"),
        }
    }
}

#[test]
fn result_names_cover_the_table() {
    assert_eq!(result_name(0), "SUCCESS");
    assert_eq!(result_name(8), "COOLDOWN_TIME");
    assert_eq!(result_name(0x1e), "INVALID_SKILL");
    assert_eq!(result_name(0x2f), "PROTECTED_FACTION");
    assert_eq!(result_name(0x84), "EXCEPT_READY_QUEST_CONTEXT");
    assert_eq!(result_name(0x7f), "URK_UNKNOWN");
}

fn req_unit_from(value: &Value) -> ReqUnit {
    if value.is_null() {
        return ReqUnit::default();
    }
    let stats = value["stats"].as_array().expect("stats");
    let mut stat_values = [0i32; 4];
    for (index, stat) in stats.iter().take(4).enumerate() {
        stat_values[index] = i32_of(stat);
    }
    ReqUnit {
        id: u32_of(&value["id"]),
        attached: value["attached"].as_bool().unwrap_or(false),
        rider: value["rider"].as_bool().unwrap_or(false),
        kind: i32_of(&value["kind"]),
        owner: u32_of(&value["owner"]),
        class_id: i32_of(&value["class_id"]),
        req_level: i32_of(&value["req_level"]),
        stat_3a7c: i32_of(&value["stat_3a7c"]),
        stat_3a80: i32_of(&value["stat_3a80"]),
        stats: stat_values,
        relation: u32_of(&value["relation"]),
        buffs: Vec::new(),
    }
}

#[test]
fn requirement_evaluators_match_goldens() {
    let doc = load("requirements");
    let cases = doc["cases"].as_array().expect("cases");
    assert!(cases.len() >= 600, "at least 20 cases per evaluator kind");
    for (index, case) in cases.iter().enumerate() {
        let kind = i32_of(&case["kind"]);
        let values: [i32; 8] = {
            let array = case["values"].as_array().expect("values");
            let mut values = [0i32; 8];
            for (slot, value) in values.iter_mut().zip(array.iter()) {
                *slot = i32_of(value);
            }
            values
        };
        let caster = case["caster_block"].as_array().expect("caster block");
        let mut block = [0u32; 8];
        for (slot, value) in block.iter_mut().zip(caster.iter()) {
            *slot = u32_of(value);
        }
        let helpers = case["helpers"].as_object().expect("helpers");
        let mut map = std::collections::BTreeMap::new();
        for (key, value) in helpers {
            map.insert(key.clone(), i32_of(value));
        }
        let local = req_unit_from(&case["ctx"]["local"]);
        let target_snapshot = req_unit_from(&case["ctx"]["target"]);
        let target = if case["ctx"]["target_ptr"].as_bool().unwrap_or(false) {
            Some(target_snapshot.clone())
        } else {
            None
        };
        let ctx = ReqCtx {
            local_id: local.id,
            local: Some(local.clone()),
            source: Some(local.clone()),
            target: target.clone(),
            source_block: ReqBlock { raw: block },
            target_block: ReqBlock::default(),
            flags: u32_of(&case["flags"]),
            units: vec![target_snapshot],
            items: Vec::new(),
            desc_target_type: 0,
            desc_flag_1a1: false,
            desc_range: 0,
            helpers: map,
            now_ms: 0,
            filetime: 0,
        };
        let row = ReqRow {
            kind,
            values,
            applies: case["applies"].as_bool().unwrap_or(true),
            row_id: 0,
        };
        let got = evaluate_requirements(&[row], &ctx);
        let want = case["result"].as_array().expect("result");
        assert_eq!(
            (got.result, got.text_id, got.value),
            (u32_of(&want[0]) as u8, u32_of(&want[1]) as u16, u32_of(&want[2])),
            "requirement case {index}: kind {kind:#x} sub {:#x} values {values:?}",
            values[0]
        );
    }
}

fn tab_actor_from(value: &Value) -> TabActor {
    let pos = value["pos"].as_array().expect("pos");
    TabActor {
        id: u32_of(&value["id"]),
        pos: [pos[0].as_f64().unwrap() as f32, pos[1].as_f64().unwrap() as f32,
              pos[2].as_f64().unwrap() as f32],
        attached: value["attached"].as_bool().unwrap_or(false),
        rider: value["rider"].as_bool().unwrap_or(false),
        kind: i32_of(&value["kind"]),
        hidden: value["hidden"].as_bool().unwrap_or(false),
        stealth: value["stealth"].as_bool().unwrap_or(false),
        stealth_visible: value["stealth_visible"].as_bool().unwrap_or(false),
        relation: u32_of(&value["relation"]),
        selectable: value["selectable"].as_bool().unwrap_or(false),
    }
}

fn tab_cfg_from(value: &Value) -> TargetingConfig {
    TargetingConfig {
        round_dist: value["round_dist"].as_f64().unwrap() as f32,
        fan_dist: value["fan_dist"].as_f64().unwrap() as f32,
        fan_angle_deg: value["fan_angle_deg"].as_f64().unwrap() as f32,
        z_limit: value["z_limit"].as_f64().unwrap() as f32,
        history_expire_s: value["history_expire_s"].as_f64().unwrap() as f32,
        history_max: u32_of(&value["history_max"]) as usize,
        dir: i32_of(&value["dir"]),
    }
}

#[test]
fn targeting_candidates_match_goldens() {
    let doc = load("targeting_collect");
    let cases = doc["cases"].as_array().expect("cases");
    assert!(cases.len() >= 20);
    for (index, case) in cases.iter().enumerate() {
        let cfg = tab_cfg_from(&case["cfg"]);
        let local_id = u32_of(&case["local_id"]);
        let actors: Vec<TabActor> = case["actors"].as_array().expect("actors")
            .iter().map(tab_actor_from).collect();
        let local = actors.iter().find(|a| a.id == local_id).expect("local actor");
        let mut state = TabState::new(if case["mode"].as_str() == Some("hostile") {
            STATE_HOSTILE
        } else {
            STATE_FRIENDLY
        });
        state.seen.clear();
        let camera = case["camera"].as_array().expect("camera");
        let local = TabActor {
            pos: [camera[0].as_f64().unwrap() as f32, camera[1].as_f64().unwrap() as f32,
                  camera[2].as_f64().unwrap() as f32],
            ..*local
        };
        let forward = case["forward"].as_array().expect("forward");
        let forward = [forward[0].as_f64().unwrap() as f32, forward[1].as_f64().unwrap() as f32];
        let got = collect_candidates(&state, &local, &actors, forward, &cfg);
        let want = case["candidates"].as_array().expect("candidates");
        assert_eq!(got.len(), want.len(), "collect case {index}");
        for (slot, expected) in got.iter().zip(want.iter()) {
            let pair = expected.as_array().expect("pair");
            assert_eq!(slot.id, u32_of(&pair[0]), "collect case {index} id");
            let distance = pair[1].as_f64().unwrap() as f32;
            assert!((slot.distance - distance).abs() < 1e-3,
                    "collect case {index} distance {} vs {distance}", slot.distance);
        }
    }
}

#[test]
fn targeting_selection_matches_goldens() {
    let doc = load("targeting_select");
    let cases = doc["cases"].as_array().expect("cases");
    assert!(cases.len() >= 20);
    for (index, case) in cases.iter().enumerate() {
        let cfg = tab_cfg_from(&case["cfg"]);
        let local_id = u32_of(&case["local_id"]);
        let actors: Vec<TabActor> = case["actors"].as_array().expect("actors")
            .iter().map(tab_actor_from).collect();
        let camera = case["camera"].as_array().expect("camera");
        let forward = case["forward"].as_array().expect("forward");
        let forward = [forward[0].as_f64().unwrap() as f32, forward[1].as_f64().unwrap() as f32];
        let mut state = TabState::new(if case["mode"].as_str() == Some("hostile") {
            STATE_HOSTILE
        } else {
            STATE_FRIENDLY
        });
        state.last_time = case["last_time"].as_f64().unwrap() as f32;
        let now = case["now"].as_f64().unwrap() as f32;
        let got = select_next(&mut state, &cfg, now, local_id, |state| {
            let local = actors.iter().find(|a| a.id == local_id).expect("local");
            let local = TabActor {
                pos: [camera[0].as_f64().unwrap() as f32, camera[1].as_f64().unwrap() as f32,
                      camera[2].as_f64().unwrap() as f32],
                ..*local
            };
            collect_candidates(state, &local, &actors, forward, &cfg)
        });
        let want = u32_of(&case["selected"]);
        assert_eq!(got.unwrap_or(0), want, "select case {index}");
    }
}

#[test]
fn tab_entry_matches_goldens() {
    let doc = load("targeting_tab");
    let cases = doc["cases"].as_array().expect("cases");
    assert!(cases.len() >= 20);
    for (index, case) in cases.iter().enumerate() {
        let cfg = tab_cfg_from(&case["cfg"]);
        let local_id = u32_of(&case["local_id"]);
        let actors: Vec<TabActor> = case["actors"].as_array().expect("actors")
            .iter().map(tab_actor_from).collect();
        let camera = case["camera"].as_array().expect("camera");
        let forward = case["forward"].as_array().expect("forward");
        let forward = [forward[0].as_f64().unwrap() as f32, forward[1].as_f64().unwrap() as f32];
        let mode = if u32_of(&case["byte"]) == 0 { TabMode::Hostile } else { TabMode::Friendly };
        let mut states = TabStates::new();
        let local = actors.iter().find(|a| a.id == local_id).expect("local");
        let local = TabActor {
            pos: [camera[0].as_f64().unwrap() as f32, camera[1].as_f64().unwrap() as f32,
                  camera[2].as_f64().unwrap() as f32],
            ..*local
        };
        let outcome = tab_select(mode, &mut states, &cfg, 0.0, &local, true, |state| {
            collect_candidates(state, &local, &actors, forward, &cfg)
        });
        let want = u32_of(&case["selection"]);
        assert_eq!(outcome.target.unwrap_or(0), want, "tab case {index}");
    }
}

#[test]
fn target_setter_and_click_match_goldens() {
    let doc = load("target_set");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let target = u32_of(&case["target"]);
        let selection = u32_of(&case["selection"]);
        let set = TargetSet {
            unit_id: u32_of(&case["unit_id"]),
            old_target: u32_of(&case["before"]),
            new_target: target,
            local: true,
            target_present: target != 0,
            frame_events: [None, None],
        };
        let result = set.apply();
        assert_eq!(result.target, target, "target set {index}");
        let resolved = match target {
            0 => 0,
            id if id == selection => id,
            _ => 0,
        };
        assert_eq!(resolved, selection, "target set selection {index}");
    }
    let doc = load("click_select");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let hover = u32_of(&case["hover"]);
        let selection = u32_of(&case["selection"]);
        let click = ClickSelect {
            hover,
            hover_aux: u32_of(&case["aux"]),
            current: u32_of(&case["before"]),
            current_aux: 0,
            hover_present: selection != 0 || hover == 0,
            can_select: true,
            mouse_clear_targeting: false,
        };
        let got = click.apply().unwrap_or(u32_of(&case["before"]));
        assert_eq!(got, hover, "click case {index}");
        let resolved = if hover == selection { selection } else { 0 };
        assert_eq!(resolved, selection, "click selection {index}");
    }
}

#[test]
fn combat_state_matches_goldens() {
    let doc = load("combat_state");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let mut byte = CombatByte { value: u32_of(&case["before"]) != 0 };
        match case["fn"].as_str().expect("fn") {
            "combat_set" => {
                byte.engage();
            }
            "combat_clear" => {
                byte.clear();
            }
            other => panic!("unknown combat fn {other}"),
        }
        assert_eq!(u32::from(byte.value), u32_of(&case["after"]), "combat case {index}");
    }
}

#[test]
fn first_hit_matches_goldens() {
    let doc = load("first_hit");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let mut unit = FirstHit::default();
        set_first_hit(&mut unit, u32_of(&case["hitter"]), u32_of(&case["team"]));
        let after = case["after"].as_array().expect("after");
        let word = |offset: usize| {
            u32_of(&after[offset])
                | (u32_of(&after[offset + 1]) << 8)
                | (u32_of(&after[offset + 2]) << 16)
                | (u32_of(&after[offset + 3]) << 24)
        };
        assert_eq!(unit.hitter, word(0), "first hit {index} hitter");
        assert_eq!(unit.team, word(4), "first hit {index} team");
    }
}

#[test]
fn force_attack_matches_goldens() {
    let doc = load("force_attack");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        if case.get("fn").and_then(|value| value.as_str()) == Some("send") {
            let byte = u32_of(&case["byte"]) != 0;
            let body = case["body"].as_array().expect("body");
            assert_eq!(
                ForceAttackRequest::build(byte).body.as_slice(),
                &[u32_of(&body[0]) as u8],
                "force send {index}"
            );
            continue;
        }
        let packet_on = u32_of(&case["packet_on"]) != 0;
        let set = ForceAttackSet {
            unit_id: 0,
            on: packet_on,
            resolved: true,
        };
        assert_eq!(set.apply(), Some(0xe8), "force set event {index}");
        assert_eq!(u32::from(packet_on), u32_of(&case["after"]), "force set byte {index}");
    }
}

fn combat_unit_from(value: &Value, in_combat: bool) -> CombatUnit {
    CombatUnit {
        id: u32_of(&value["id"]),
        attached: value["attached"].as_bool().unwrap_or(false),
        rider: value["rider"].as_bool().unwrap_or(false),
        kind: i32_of(&value["kind"]),
        owner: u32_of(&value["owner"]),
        in_combat,
    }
}

#[test]
fn engage_matches_goldens() {
    let doc = load("engage");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let source = combat_unit_from(&case["source"], false);
        let target = combat_unit_from(&case["target"], false);
        let engaged = engage_combat(
            i32_of(&case["damage"]),
            u32_of(&case["engage"]) != 0,
            Some(source),
            Some(target),
            None,
            0,
        );
        let source_after = engaged.contains(&source.id);
        let target_after = engaged.contains(&target.id);
        assert_eq!(source_after, u32_of(&case["source_after"]) != 0, "engage {index} source");
        assert_eq!(target_after, u32_of(&case["target_after"]) != 0, "engage {index} target");
    }
}

#[test]
fn buff_append_matches_goldens() {
    let doc = load("buff_append");
    for (index, case) in doc["cases"].as_array().expect("cases").iter().enumerate() {
        let kind = i32_of(&case["kind"]);
        let mut lists = BuffLists::default();
        let appended = append_buff(&mut lists, kind, 1);
        let row = u32_of(&case["row"]);
        let offset = match appended {
            Some(BuffListKind::Buff) => {
                assert_eq!(lists.buffs.len(), 1);
                0x28
            }
            Some(BuffListKind::Debuff) => {
                assert_eq!(lists.debuffs.len(), 1);
                0xd30
            }
            Some(BuffListKind::Hidden) => {
                assert_eq!(lists.hidden.len(), 1);
                0x16f8
            }
            None => 0,
        };
        assert_eq!(offset, row, "buff append {index} row");
        let counts = case["counts"].as_array().expect("counts");
        assert_eq!(u32_of(&counts[0]), u32::from(lists.buffs.len() == 1), "buffs {index}");
        assert_eq!(u32_of(&counts[1]), u32::from(lists.debuffs.len() == 1), "debuffs {index}");
        assert_eq!(u32_of(&counts[2]), u32::from(lists.hidden.len() == 1), "hidden {index}");
    }
}
