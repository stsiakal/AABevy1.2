import csv, os, re, collections

LO, HI = 0x38ffdc0b, 0x3926de70
ROOT = r'C:/AAEMU_DISK/aaemu-client/wt/map3'
BASE = r'D:/aa-re/client12/out-x2game'
OUT = os.path.join(ROOT, 'docs/research/x2game-survey/r1-38ffdc0b.md')

# (start, area, status, runtime, what, evidence)
AREAS = [
    (0x38ffdc0b, 'Startup / unpacked-tail exception fragments', 'UNKNOWN', 'unknown',
     'Fragments that jump into the 0x3a6xxxxx unpacked tail (SEH/EBP setup); no strings',
     'no strings; decompile shows EBP fragments and jumps into 0x3a6a1977/0x3ab943a8 [R]'),
    (0x39000000, 'CAIHandler / AI script state', 'MAPPED', 'yes (anchors only)',
     'Loads aicharacter_character / aibehavior_behaviour / AVAILABLE, owns the per-entity AI script object; client registers anchors, not NPC puppets',
     'port-engine-ai/MAP.md §7.5 (0x39002d10 ctor, 0x39003000/0x39003640 dtor, 21 fns cited); <CAIHandler> strings [R]'),
    (0x39004000, 'EnvironmentManager weather + RelicEffect flow node', 'COVERED', 'yes',
     'Weather/time-of-day manager update (sunny.xml), relic effect flow node and nearby game helpers',
     'X2::EnvironmentManager::Update 0x39005400, game/libs/flownodes/weather/sunny.xml; aa-world sky/ToD, port-flowgraph/nodes.md entity:RelicEffect [R]'),
    (0x39006000, 'Game-object/unit script parameters', 'COVERED', 'yes',
     'gimmick_%u naming, SetParamX2 and object parameter plumbing used by the Lua game layer',
     'strings gimmick_%u / SetParamX2 at 0x39007290/0x39007a10; aa-lua script bind + aa-client gimmick [R/I]'),
    (0x39008000, 'Entity / rope wrappers', 'COVERED', 'yes',
     'Entity debug names, rope/follower creation and boxes/launchers for visual ropes',
     'strings <Entity : %s>, <Rope>, <Foliage>, <Water Volume>, $rope_anchor, %s_rope, objects/box.cgf at 0x39008330/0x390089b0 [R]; aa-world entities, aa-fx rope'),
    (0x3900a000, 'Lua Data table conversion', 'COVERED', 'yes',
     'Converts engine Data records to/from Lua tables (isTable, str_/num_/bool_ keys, enum cases)',
     'Data::SetAttribute("isTable",true), str_%s/num_%d/bool_%s at 0x3900aa60 [R]; aa-lua / aa-ui argument conversion'),
    (0x3900c000, 'Achievement tracking and item-sent event', 'COVERED', 'yes',
     'Achievement tracing flags and ACHIEVEMENT_ITEM_SENT event fan-out',
     'tracing_achievement 0x3900cac0, ACHIEVEMENT_ITEM_SENT 0x3900e3f0 [R]; aa-client src/hud/achievement.rs, aa-net achievement'),
    (0x39010000, 'Prefab/asset path helpers + TimeProfiler', 'COVERED', 'yes; profiler is debug',
     'prefab://, prefabs/, .xml/ path building and a TimeProfiler log helper',
     'prefab:// at 0x39010c00, TimeProfiler/[%s] - (%8.2f ms) at 0x39010930 [R]; aa-world prefab paths; profiler OUT'),
    (0x39014000, 'GameAssetManager prefab cache', 'COVERED', 'yes',
     'InitPrefabCache and asset/prefab lookup used by the client asset manager',
     'X2::GameClient::GameAssetManager::InitPrefabCache 0x390140c0 [R]; aa-world/aa-cry prefab loading'),
    (0x39015000, 'Housing inventory / placement + async UI events', 'MAPPED', 'yes',
     'Housing inventory limits, placement checks, building/demolish setup and AsyncUiEventQueue',
     'port-housing/MAP.md + inventory.md (0x390155a0, 0x39016fe0, 0x39017220, 0x39017410, 0x39017620), port-interaction/functions.md; 13 fns cited [R]'),
    (0x3901a000, 'UI message boxes, unit clone, string helpers', 'COVERED', 'yes',
     'Sea warning / error message box, pc_clone handling and bounded string formatting used by UI',
     'msg_box_warning_sea/error_message 0x3901c540, pc_clone 0x3901bcd0 [R]; aa-ui Lua message boxes, aa-client'),
    (0x3901e000, 'Quest-mark prefab manager', 'COVERED', 'yes',
     'Loads objects/ui/quest_mark/prologue_arrow and AnimationPrefabObject quest markers',
     'objects/ui/quest_mark/...cga + AnimationPrefabObject 0x3901eb30 [R]; aa-client src/hud/marks.rs + quests.rs'),
    (0x39020000, 'Client-driven instance scene manager', 'TO-PORT', 'yes',
     'clientdriven.g scene table (sceneId/childeSceneId, permitted widgets/keys), prologue water check, RequestSkipClientDrivenIndun',
     'game/clientdriven.g, failed set scene info..., RequestSkipClientDrivenIndun() at 0x39021c00/0x39022c10/0x390235b0 [R]; no port map and no aa-client scene-info code; aa-ui only stubs [I]'),
    (0x39024000, 'AI aggro / alert / goal events', 'MAPPED', 'yes (dormant NPC puppets)',
     'GO_TO_DEAD goal, OnAggroTargetChanged / OnAlertTargetChanged emitters and AI script event dispatch',
     'port-engine-ai/MAP.md §7.7 (0x39023c30, 0x39023ce0, 0x39024540, 0x390276e0) [R]'),
    (0x3902a000, 'Zone state / anim LOD / data-mining helpers', 'COVERED', 'yes; mining is telemetry',
     'zone remainTime/honorPointSum/zoneName, ca_MergeMaxNumLods read, DataMiningManager UUID',
     'remainTime/honorPointSum/zoneName 0x39029920, ca_MergeMaxNumLods 0x3902a8a0, DataMiningManager 0x3902af20 [R]; aa-client zone data, aa-anim LOD; DataMining OUT'),
    (0x3902b000, 'UI dialog / window manager', 'COVERED', 'yes',
     'Dialog/window construction and lookup by title/content/button (btnOk, btnCancel, textbox, isModal)',
     'title/content/button/dialog/window/btnOk/btnCancel/isModal at 0x3902b980/0x3902d270/0x3902d400 [R]; aa-ui widget/window layer'),
    (0x39030000, 'DoodadManager / farm collision', 'MAPPED', 'yes',
     'Doodad stream/create/phase/LOD/name-tag handling, farm collision and housing doodad helpers',
     'port-housing/inventory.md (0x390302f0..0x3903b930 incl. 0x390382d0 HandleDoodadCreated), port-interaction/functions.md (0x39035280) [R]'),
    (0x3903c000, 'Housing decor placement raycast', 'MAPPED', 'yes',
     'Furniture/decor placement, overlap checks and ray tests on housing doodads',
     'port-housing/MAP.md + checks.md + classes.md (0x3903c2e0, 0x3903c5a0, 0x3903d6a0..0x3903e080); housing/decor.rs [R]'),
    (0x3903e100, 'In-game cash shop data model', 'COVERED', 'yes',
     'Maintab/subtab/goods browsing, checkTime, bonus/event/hot/new categories and cash-shop UI data',
     'goods/subtab/maintab/checkTime 0x3903e9d0/0x39040fc0/0x39041550; OnInGameCashShopCheckTime string [R]; aa-net cash_shop.rs, aa-client hud/store'),
    (0x39044000, 'Item info / tooltip / enchant / socket tables', 'COVERED', 'yes',
     'Item grade enchant costs, socketing, dyeability, corpse_info and item tooltip field tables',
     'grade_enchant_currency/cost, socketing, unsocket, isDyeable/useAsSkin/lookType, corpse_info at 0x39044d30..0x39048dc0 [R]; aa-client hud item tooltip; port-formulas/port-combat mapped leaves'),
    (0x39048000, 'World-map markers / minimap / portals', 'COVERED', 'yes',
     'M_* marker table (NPC stores, quests, houses, portals, party), map mask and portal markers',
     'port-portals/functions.md 0x3904ce90 minimap marker table (index 0x45 = M_PORTAL), 0x3904c690/0x3904c760/0x39051de0 [R]; marker behaviour in aa-client src/hud/markers.rs/radar.rs'),
    (0x39050000, 'MusicManager / zone music state', 'COVERED', 'yes',
     'MusicManager::Update, zone/sub-zone music pack selection and music state',
     'X2::GameClient::MusicManager::Update 0x39052ea0; port-engine-loop/MAP.md cites it for update order; aa-client src/sound/music.rs + zone.rs explicitly port MusicManager/zone tracks [R]'),
    (0x39054000, 'Lua API batch: music, combat text, achievements, books, buffs, beauty shop, UCC', 'COVERED', 'yes',
     'Native X2 Lua getters/actions for damage/heal text, achievements, books, buff skills, beauty-shop custom data and UCC files',
     'music/damage/heal 0x39057410; achievement API 0x3905a100; book 0x3905ab50; buff skills 0x3905b510; custom data 0x3905d7e0 [R]; aa-ui Lua runtime, aa-client hud/achievement, aa-audio'),
    (0x39060000, 'Cash-shop / premium-shop Lua API', 'COVERED', 'yes',
     'Price types (cash/AA point/BM mileage), limited sales, packages, premium service and buy windows',
     'priceType/itemType/afterCash/afterAAPoint 0x39060f40/0x39062710; PSBM_*/ViewBuyCashWindow 0x39072430 [R]; aa-net cash_shop.rs, aa-ui stubs, aa-client hud/store'),
    (0x39070000, 'Premium service / rankings / web-store Lua API', 'COVERED', 'yes',
     'Premium grades, skill/unit modifier menus, appellation/buff info, rank scopes/snapshots and web buy URL',
     'PSBM_/PG_PREMIUM_/RequestRankSnapshots 0x39072430/0x39076010; http://%s/store/buy?uuid=%s 0x39071c90 [R]; aa-client hud/ranking + net; web browser path not ported [I]'),
    (0x39078000, 'Sound moods / client trial / user music', 'COVERED', 'yes',
     'SoundMoodManager, ClientTrialManager restrictions and USER/midi/AA_%d.mid user music',
     'SoundMoodManager::Update 0x3907b3c0, ClientTrialManager::Update 0x3907ce50, USER/midi/AA_%d.mid 0x39082310; TSS 0x39081320/0x390815c0 [R]; aa-client sound/moods.rs + hud/trial.rs, aa-audio'),
    (0x39084000, 'Weather: rain + tornado flow nodes', 'COVERED', 'yes',
     'EngineAssets/raintextures.xml, tornado wander targets and wind.tornado.* emitters',
     'EngineAssets/raintextures.xml 0x39085c90, wind.tornado.* 0x39087710/0x39088410, tornado node help 0x39085030 [R]; aa-world weather/sky, aa-fx wind/rain'),
    (0x39088000, 'Housing vegetation / farm fields', 'MAPPED', 'yes',
     'Crop/grapevine assets, FarmFieldModel growing steps and housing vegetation erase',
     'port-housing/inventory.md (0x3908c790..0x3908e540), X2::HousingBase::EraseVegetation 0x3908e040, FarmFieldModel::SetGrowingStep [R]'),
    (0x3908c000, 'Housing connectors / fences', 'MAPPED', 'yes',
     'Loads housing connectors (gate/door/stair/height) and fences for placed houses',
     'port-housing/inventory.md 0x390904b0 (13 KB connector loader), 0x39093800/0x39093860; callers Housing::LoadConnectors [R]'),
    (0x39094000, 'Client skill controller', 'MAPPED', 'yes',
     'Net Floating Skill Controller: client-side skill effect/float state',
     'X2:SkillController / Net Floating Skill Controller strings 0x39094000 bins; port-combat/MAP.md and functions.md (14 fns cited) [R]'),
    (0x3909c000, 'Wheeled vehicle controller', 'MAPPED', 'yes',
     'Wheeled vehicle warp/reset and driving controller glue',
     'wheeled vehicle warped from... 0x3909c000 bin; port-vehicles/functions.md 0x3909c000-0x390a0000 (incl. 0x390cf180 later) [R]'),
    (0x390a0000, 'Net actor movement controller', 'MAPPED', 'yes',
     'Remote actor position/velocity smoothing and living-entity update',
     'X2::NetActorMovementController::Update/UpdateLivingEntity/PrePhysicsUpdate 0x390a30e0/0x390a3bb0/0x390af040 [R]; port-movement remote smoothing + port-movement-extra'),
    (0x390a8000, 'Rope skill controllers + movement queue', 'MAPPED', 'yes',
     'Skill rope/launcher controllers and the unit movement queue overflow path',
     'rope_skill_controller_end_rope / objects/box.cgf / Game/materials/misc/rope/rope_a 0x390b04f0, movement queue over flowed 0x390b0000 bin [R]; aa-fx rope, port-movement functions.md'),
    (0x390b0000, 'Unit core: attach/detach, model swap, mana', 'MAPPED', 'yes',
     'Unit::AttachTo/DetachFrom, model init/change/release/state, mana regen and unit model manager',
     'X2::Unit::* 0x390b3f10..0x390ba9c0 [R]; port-engine-char/port-mount-anim B-mount-dismount + port-vehicles (24 fns cited around 0x390b2000)'),
    (0x390c4000, 'UnitModel classes: actor / prefab / ship / vehicle', 'MAPPED', 'yes',
     'ActorUnitModel, PrefabUnitModel, ShipUnitModel and VehicleUnitModel init/update/effects',
     'X2::*UnitModel::* 0x390c4270..0x390ec1c0 [R]; port-engine-anim-char/MAP.md, port-mount-anim, port-vehicles/functions.md'),
    (0x390e0000, 'UnitModel dtors / manager / update', 'MAPPED', 'yes',
     'UnitModel/Empty/Actor/Prefab/Vehicle destructors, UnitModelManager::Create and update',
     'X2::UnitModel::~UnitModel etc 0x390e43f0..0x390ec1c0 [R]; port-engine-anim-char/MAP.md, port-mount-anim'),
    (0x390f0000, 'CGame init / prephysics / option config', 'MAPPED', 'yes',
     'CGame::Init/CompleteInit, PrePhysicsUpdate, option and auth-server cvar reads, input console',
     'port-engine-loop/MAP.md (0x390f5360), port-program.md; 0x390f7170 CGame::Init, 0x390f7b40 option_sound/auth_server/expr_mode cited by port-movement-extra/constants.md [R]'),
    (0x390f8000, 'Environment prefabs, PrefabObject loaders, GameTokens', 'COVERED', 'yes',
     'Rain/Tornado prefab scripts, AIPoint/Brush/EntitySlot/ParticleEffect prefab objects and game tokens/shared-memory store token',
     'Scripts/Entities/Environment/Rain.lua/Tornado.lua 0x390f8000; AIPointPrefabObject::CreateNode 0x390ff290; <storeToken> 0x390fc000 [R]; aa-world prefab loader, aa-fx; AIPoint MAPPED port-engine-ai'),
    (0x39100000, 'Prefab loader / objects (world assets)', 'COVERED', 'yes',
     'Prefab::Load*/PrecacheLibs, VisArea, EntitySlot, ParticleEffect, WaterVolume and AI nav modifier objects',
     'Prefab::* 0x3910e690..0x391114f0; VisAreaPrefabObject 0x39110770; port-engine-survey/audit-world.md; port-engine-ai MAP AINavModifier [R]; aa-world/aa-cry'),
    (0x39114000, 'CActor / CPlayer / Monster / Quadruped movement', 'MAPPED', 'yes',
     'Actor update/movement request, player frame movement and rotation, monster/quadruped prephysics, stats',
     'port-movement/MAP.md + functions.md, port-movement-extra, port-mount-anim, port-engine-anim-char; ~90 fns cited across 0x39114000-0x39141000 [R]'),
    (0x39144000, 'Character effect params + flow-graph nodes', 'MAPPED', 'yes',
     'Effect param structs (attachName/boneName/hitPos/damage), sound moods, camera/FOV and CryAction-derived flow nodes',
     'port-flowgraph/MAP.md + functions.md/nodes.md (0x3914a000-0x39152000), port-engine-combat; strings Enable/Disable HitInfo, Override Camera FOV [R]'),
    (0x39154000, 'AnimationStateMachine', 'MAPPED', 'yes',
     'Animation state-machine descriptor manager, wrapper map and queue actor binding',
     'X2::AnimationStateMachine::* 0x39159c20..0x3915a5a0 [R]; port-engine-anim + port-mount-anim D-graph-runtime (19 mapped / 14 ported in 0x39154000)'),
    (0x3915c000, 'Bokeh / lens textures', 'OUT', 'yes (rendering)',
     'Bokeh circle/heart/hexagon/star textures and camera post-effect helpers',
     'Game/ui/bokeh/*.dds strings 0x3915c000 [R]; Bevy/renderer owns post effects'),
    (0x39160000, 'Camera manager + camera/input math', 'MAPPED', 'yes',
     'CameraManager update/fade/actual distance, camera collision rays and action/key-name tables',
     'port-movement/functions.md 0x3916aab0 CameraManager::Update; movement/camera.rs ported; 41 fns in 0x39160000 bin, 12 mapped/4 ported [R]'),
    (0x3916c000, 'Custom textures, GameClient prephysics, name tags', 'COVERED', 'yes',
     'Ladder/climb clips, face/body CustomTextureManager (UCC), GameClient::PrePhysicsUpdate and name-tag cvars',
     'X2::GameClient::CustomTextureManager::* 0x3916d960/0x3916f970; X2::GameClient::PrePhysicsUpdate 0x39171590; aa-cry static UCC (dynamic crest binding missing), aa-client prephysics/hud/plates'),
    (0x39174000, 'GameClient bootstrap, gimmick/model streaming/physics', 'COVERED', 'yes',
     'GameClient::Initialize, ClientGimmickManager, ModelStreamingManager/Task, PhysicsManager and raycast flags',
     'X2::GameClient::Initialize 0x39174520, ClientGimmickManager 0x39179ea0, ModelStreamingManager 0x3917bbf0, PhysicsManager 0x39181de0 [R]; aa-client wiring, aa-world streaming, aaemu-physics'),
    (0x39184000, 'Packet senders / item links / world list', 'COVERED', 'yes',
     'Client packet send wrappers (item link/socket/enchant results, world list, options)',
     'item link strings 0x39185bd0/0x39185d70; trying to go to world list 0x39185930; port-combat/packets.md + port-portals/packets.md mapped leaves [R]; aa-net'),
    (0x39188000, 'SC packet handler / dispatch classes', 'COVERED', 'yes',
     '~2,039 tiny vtable/destructor bodies for server-to-client packet handler classes (no strings)',
     'uniform 21/23/31/80/95-byte bodies and packet vtable pointers 0x399b2e1c/0x399b30xx; neighbouring handlers log SC packet names [R/I]; aa-net protocol'),
    (0x391a4000, 'Character creation + auction Lua bindings', 'COVERED', 'yes',
     'Character-create texts (title/content, error_char_name_format, fist_ac_choice) and auction post/bid/search levels',
     'title/content/error_char_name_format/fist_ac_choice 0x391a40d0/0x391a4390; postLevel/bidLevel/searchLevel/limitLevels 0x391a5100 [R]; aa-client login/create + hud/auction'),
    (0x391a8000, 'CS packet send wrappers', 'COVERED', 'yes',
     '~469 uniform 122-byte wrappers that build a packet class, serialize via vtable+8 and send',
     'PTR_LAB_399b30fc + PTR_FUN_399b6340 construction at 0x391a8050 [R]; aa-net client messages'),
    (0x391b8000, 'Network dispatch / packet handlers', 'COVERED', 'yes',
     'RegisterPacketHandler table (7.8 KB at 0x391eb9d0) and handlers for unit attach, gimmick, level, item, coffer, cutscene gates',
     'client OnUnitAttached/OnUnitDetached 0x391d4000; client received gimmick ... 0x391d8000; registration 0x391eb9d0 [R]; aa-net session handlers'),
    (0x391e8000, 'ClientPlayer / SubZone / Tutorial / notifier', 'COVERED', 'yes',
     'ClientPlayer::Update, SubZoneManager::Update, Tutorial quest-area events and GameEventNotifier',
     'ClientPlayer 0x391e27f0, SubZoneManager 0x391ef3b0, Tutorial 0x391f0250, GameEventNotifier::Notify 0x39202db0 [R]; aa-client hud, aa-world subzone'),
    (0x39200000, 'Game event dispatch / notifications', 'COVERED', 'yes',
     'GameEventNotifier::Notify fan-out to HUD/UI events',
     'X2::GameClient::GameEventNotifier::Notify 0x39202db0 and callers OnLevelChanged/OnCofferContents [R]; aa-client hud api events'),
    (0x39203000, 'Notification / report helpers', 'COVERED', 'yes',
     'Notification queue helpers around the event notifier',
     '0x39203020..0x39203dc0 functions; deque<T>; called by UI update [R/I]; aa-client hud'),
    (0x39203e90, 'TQOS telemetry report', 'OUT', 'no (telemetry)',
     'Sends TQOS reports to ied-tqos.qq.com',
     'Failed to send TQOS report!, tqos_create_api_handle, ied-tqos.qq.com at 0x39203e90 [R]'),
    (0x39204000, 'Cash-shop icons / skill info UI data', 'COVERED', 'yes',
     'Cash-shop and skill-info icon paths and UI content data',
     'ui/icon/icon_skill_info_view.dds, icon_skill_vehicle03.dds 0x39204000 [R]; aa-ui assets/hud'),
    (0x39208000, 'Buff/item/mate/skill accessors + UIContentsManager', 'COVERED', 'yes',
     'Client accessors that project buffs, items, mate/slave skills and skills into UI data',
     'X2::GameClient::*Accessor::Update 0x3920a320..0x3920c320 [R]; aa-client hud api/state'),
    (0x39210000, 'UIContentsManager scripts / UI leveldb', 'COVERED', 'yes',
     'LoadBaseLib/LoadGlobalUI, scripts/x2ui and UI data leveldb persistence',
     'UIContentsManager 0x392128c0/0x39212940, leveldb init/save/load 0x39214000 [R]; aa-ui Lua runtime, aa-client ui_data/settings'),
    (0x39214000, 'UI event registration table + leveldb', 'COVERED', 'yes',
     'Registers hundreds of UI events (DISCONNECTED_BY_WORLD, SENSITIVE_OPERATION_VERIFY, STORE_TRADE_FAILED, UPDATE_CLIENT_DRIVEN_INFO) and stores UI data',
     '0x39214380 (9.4 KB registration table), leveldb strings [R]; aa-ui event runtime, aa-client ui_data'),
    (0x39218000, 'DesktopListener UI input, fonts/textures', 'COVERED', 'yes',
     'Desktop input event routing, font/name-tag texture counts and UI texture memory reports',
     'DesktopListener::OnInputEventUI 0x3921b910 / OnInputEvent 0x3921d6a0; font count/name tag texture memory 0x39218000 [R]; aa-ui input + Bevy text/rendering'),
    (0x39220000, 'Display-list batching', 'OUT', 'yes (rendering)',
     'Builds display lists (vertex counts) for UI drawables',
     'displaylist : %u (vert : %u) 0x39220000 [R]; renderer/Bevy owns batching'),
    (0x39224000, 'ScriptBindUI / ScriptBindWidget Lua widget API', 'COVERED', 'yes',
     'All native UI Lua bindings: widgets, bounds anchors, text drawables, tabs, list/table, item slots, UI errors/log',
     'ScriptBindUI/ScriptBindWidget/ScriptBindTextDrawable/ScriptBindUIBounds 0x392344e0..0x39268c50 [R]; aa-ui native Lua bindings'),
]

SPECIAL_OUT = {
    0x39010930: 'TimeProfiler log helper (debug)',
    0x3902af20: 'DataMiningManager (telemetry UUID/timer)',
    0x39081320: 'TSS packet encryption (anti-cheat)',
    0x390815c0: 'TSS initialization (anti-cheat)',
    0x390fcd80: 'web-store token XML (<storeToken>, no web backend)',
    0x390fd0c0: 'web-store token XML (<storeToken>, no web backend)',
    0x390fffb0: 'web-store shared-memory bridge (no web backend)',
}


def load_funcs():
    rows = []
    with open(BASE + '/funcs.csv', encoding='utf-8', errors='replace') as f:
        for x in csv.DictReader(f):
            s = int(x['start'], 16)
            if LO <= s < HI:
                rows.append({'start': s, 'size': int(x['size']), 'name': x['name']})
    rows.sort(key=lambda r: r['start'])
    return rows


HEX = re.compile(r'(?<![0-9A-Za-z_])(?:0x)?([0-9a-fA-F]{8})(?![0-9A-Za-z_])')


def load_cites():
    docs = collections.defaultdict(set)
    for root, dirs, files in os.walk(os.path.join(ROOT, 'docs', 'research')):
        base = os.path.basename(root)
        if not (base.startswith('port-') or root == os.path.join(ROOT, 'docs', 'research')):
            continue
        for fn in files:
            if not fn.endswith('.md'):
                continue
            rel = os.path.relpath(os.path.join(root, fn), os.path.join(ROOT, 'docs', 'research')).replace('\\', '/')
            if not (rel.startswith('port-') or rel == 'port-program.md'):
                continue
            text = open(os.path.join(root, fn), encoding='utf-8', errors='replace').read()
            for m in HEX.finditer(text):
                a = int(m.group(1), 16)
                if LO <= a < HI:
                    docs[a].add(rel)
    ports = collections.defaultdict(set)
    proot = os.path.join(ROOT, 'crates', 'aa-client', 'src', 'port')
    for root, dirs, files in os.walk(proot):
        for fn in files:
            if fn.endswith('.rs'):
                rel = os.path.relpath(os.path.join(root, fn), proot).replace('\\', '/')
                text = open(os.path.join(root, fn), encoding='utf-8', errors='replace').read()
                for m in HEX.finditer(text):
                    a = int(m.group(1), 16)
                    if LO <= a < HI:
                        ports[a].add(rel)
    return docs, ports


def main():
    funcs = load_funcs()
    docs, ports = load_cites()
    starts = [a[0] for a in AREAS]
    groups = collections.defaultdict(lambda: {'fns': 0, 'bytes': 0, 'cites': collections.Counter(),
                                             'port_cites': collections.Counter(), 'min': None, 'max': 0})
    for f in funcs:
        idx = 0
        for i, s in enumerate(starts):
            if f['start'] >= s:
                idx = i
            else:
                break
        start, area, astatus, runtime, what, ev = AREAS[idx]
        if f['start'] in ports:
            status = 'PORTED'
        elif f['start'] in docs:
            status = 'MAPPED'
        elif f['start'] in SPECIAL_OUT:
            status = 'OUT'
        else:
            status = astatus
        g = groups[(start, area, status, runtime, what, ev)]
        g['fns'] += 1
        g['bytes'] += f['size']
        g['min'] = f['start'] if g['min'] is None else min(g['min'], f['start'])
        g['max'] = max(g['max'], f['start'])
        for c in docs.get(f['start'], ()):
            g['cites'][c] += 1
        for c in ports.get(f['start'], ()):
            g['port_cites'][c] += 1

    totals = collections.Counter()
    total_fns = 0
    total_bytes = 0
    for (start, area, status, runtime, what, ev), g in groups.items():
        totals[status + '_fns'] += g['fns']
        totals[status + '_bytes'] += g['bytes']
        total_fns += g['fns']
        total_bytes += g['bytes']

    lines = []
    lines.append('# x2game.dll survey: r1-38ffdc0b (0x38ffdc0b .. 0x3926de70)')
    lines.append('')
    lines.append('Read-only classification of every function with `start >= 0x38ffdc0b` and '
                 '`start < 0x3926de70` in the shipped 1.2 `x2game.dll` decompile. Method follows '
                 '`docs/research/port-engine-survey/core.md` §0 / Module totals: extents and sizes from '
                 '`funcs.csv`; areas from the binary\'s own strings (profiler/class names, packet/log text, '
                 'cvars, file paths), address neighbourhoods and call clusters; existing maps linked, not repeated.')
    lines.append('')
    lines.append('Tags: **[R]** read from the binary/decompile; **[I]** inferred from a call cluster or '
                 'an unread branch; **[?]** open. Sizes are code bytes (`funcs.csv` `size`), KB = 1024 B. '
                 'Status precedence: an address cited in `crates/aa-client/src/port/**` is PORTED; an address '
                 'cited in a `docs/research/port-*/` map (or `port-program.md`) is MAPPED; otherwise the area '
                 'status applies. Only `--hidden` original clients were used for any live checks; this survey '
                 'made none (no live stack was present in the worktree).')
    lines.append('')
    lines.append('| status | functions | KB | meaning |')
    lines.append('|---|---:|---:|---|')
    for st, meaning in [
        ('PORTED', 'address found in `crates/aa-client/src/port/**`'),
        ('MAPPED', 'address/area in an existing `docs/research/port-*/` map or `port-program.md`'),
        ('COVERED', 'an existing crate implements the area; not proven equal'),
        ('TO-PORT', 'client game logic/behaviour with no map and no crate implementation'),
        ('OUT', 'rendering, debug/editor/tools, telemetry, anti-cheat or dead code'),
        ('UNKNOWN', 'could not be classified; reason given in the area table'),
    ]:
        lines.append('| %s | %d | %.1f | %s |' % (st, totals[st + '_fns'], totals[st + '_bytes'] / 1024, meaning))
    lines.append('| **total** | **%d** | **%.1f** | `funcs.csv` range total |' % (total_fns, total_bytes / 1024))
    lines.append('')
    assert total_fns == len(funcs)
    assert total_bytes == sum(f['size'] for f in funcs)
    direct_map = [f for f in funcs if f['start'] in docs and f['start'] not in ports]
    direct_port = [f for f in funcs if f['start'] in ports]
    area_map_fns = totals['MAPPED_fns'] - len(direct_map)
    area_map_bytes = totals['MAPPED_bytes'] - sum(f['size'] for f in direct_map)
    lines.append('**Direct-citation split.** MAPPED directly cited addresses: %d fns / %.1f KB; '
                 'MAPPED by the surrounding mapped area only: %d fns / %.1f KB [I]. PORTED directly: '
                 '%d fns / %.1f KB. A direct citation can be incidental re-use (a shared vtable or helper '
                 'used by another system); the other row for the same area gives the area status.' % (
                     len(direct_map), sum(f['size'] for f in direct_map) / 1024,
                     area_map_fns, area_map_bytes / 1024,
                     len(direct_port), sum(f['size'] for f in direct_port) / 1024))
    lines.append('')
    lines.append('**Totals check.** %d functions, %d bytes (%.1f KB); the function/byte columns below sum '
                 'exactly to this.' % (total_fns, total_bytes, total_bytes / 1024))
    lines.append('')
    lines.append('## Area table')
    lines.append('')
    lines.append('| area | fns | KB | what it does | runtime | status | evidence |')
    lines.append('|---|---:|---:|---|---|---|---|')
    order = {s: i for i, s in enumerate(starts)}
    for key in sorted(groups, key=lambda k: (order[k[0]], k[2])):
        start, area, status, runtime, what, ev = key
        g = groups[key]
        cites = ', '.join('%s (%d)' % (c, n) for c, n in g['cites'].most_common(3))
        pcites = ', '.join('%s (%d)' % (c, n) for c, n in g['port_cites'].most_common(2))
        evidence = ev
        if cites:
            evidence += '; cited: ' + cites
        if pcites:
            evidence += '; ported: ' + pcites
        suffix = '' if status in ('MAPPED', 'PORTED') else ''
        lines.append('| %s%s | %d | %.1f | %s | %s | %s | %s |' % (
            area, suffix, g['fns'], g['bytes'] / 1024, what, runtime, status, evidence))
    lines.append('')
    lines.append('## Largest TO-PORT and UNKNOWN areas')
    lines.append('')
    for st in ('TO-PORT', 'UNKNOWN'):
        rows = [(k, v) for k, v in groups.items() if k[2] == st]
        rows.sort(key=lambda kv: -kv[1]['bytes'])
        if not rows:
            lines.append('* **%s:** none.' % st)
            continue
        lines.append('* **%s:**' % st)
        for k, v in rows[:10]:
            lines.append('  * `%s-%s` %s - %d fns, %.1f KB (runtime: %s)' % (
                hex(v['min']), hex(v['max']), k[1], v['fns'], v['bytes'] / 1024, k[3]))
    lines.append('')
    lines.append('## What remains (client game logic no map covers)')
    lines.append('')
    lines.append('The only unmapped client behaviour of size in this slice is the **client-driven instance '
                 'scene manager** at `0x39020000-0x39023c00` (`game/clientdriven.g`, `sceneId`/`childeSceneId`, '
                 'permitted widgets/keys, `RequestSkipClientDrivenIndun`). `aa-ui` only stubs '
                 '`IsInClientDrivenZone`/`RequestEndClientDrivenIndun`; no `crates/aa-client` code reads a scene '
                 'info table. Everything else with player-visible behaviour is either in an existing map '
                 '(movement, camera, actor/unit models, housing, vehicles, doodads, portals, flow graph, anim '
                 'state machine, AI anchors) or in a crate (protocol, UI Lua runtime, world/prefabs, audio, '
                 'effects, physics).')
    lines.append('')
    lines.append('Gaps worth knowing even where the area is covered: the dynamic (per-player) UCC/crest binding '
                 'behind `CustomTextureManager` is only statically baked in `aa-cry` ("dynamic player crests '
                 'need a separate binding"); the web buy/password windows (`ViewBuyCashWindow`, '
                 '`http://%s/store/buy?uuid=%s`, `originPassword...`) have no web backend and are deliberately '
                 'left out; TSS packet encryption, TQOS reporting and DataMiningManager are anti-cheat/telemetry '
                 'and are OUT.')
    lines.append('')
    lines.append('## Script')
    lines.append('')
    lines.append('`tools/x2game_survey/` (`survey.py`, `bins.py`, `coverage.py`, `keyword_scan.py`, '
                 '`slice.py`, `strings_of.py`, `calls_of.py`, `explain.py`, `areas.py`). `areas.py` regenerates '
                 'this table and asserts the totals. Derived only from the shipped binary/decompile; no '
                 'CRYENGINE source was used.')
    lines.append('')
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, 'w', encoding='utf-8', newline='\n') as f:
        f.write('\n'.join(lines))
    print('wrote', OUT)
    print('total', total_fns, total_bytes, 'statuses', {k[:-4]: totals[k] for k in totals if k.endswith('_fns')})
    for k, v in sorted(groups.items(), key=lambda kv: -kv[1]['bytes'])[:8]:
        if k[2] in ('TO-PORT', 'UNKNOWN'):
            print(k[2], hex(k[0]), k[1], v['fns'], round(v['bytes'] / 1024, 1))


if __name__ == '__main__':
    main()
