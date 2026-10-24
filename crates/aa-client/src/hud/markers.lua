-- The icons of the roadmap and of the big map (hud/markers.rs). HUD.map_icons is the list in game coordinates
-- ({ x, y, kind, tip, zone, city }); HUD.map_quest_areas are the active quests' objective circles
-- ({ qtype, x, y, radius, zone, city }, the `area` records of the client's quest_sign_sphere.g). This file keeps
-- the "Show Icons" window's filters, the quest notifier's decal indices and colour/number tables (the natives of
-- map/common.lua and questcontext/quest_context_notifier.lua), places the icons and circles on the zone or city map
-- the roadmap or the big map shows and hands them to the toolkit: AAUI.RoadMap().markers (the place on the map
-- image) and AAUI.WorldMap().markers (the place on the map `level`, `id`). Loaded after ingame.lua, whose hooks it
-- wraps.

-- The filters ------------------------------------------------------------------------------------------------
-- map/filter.lua tells the game what to show: X2Map:SetNpcFilter(iconType, on) for each icon type, and
-- SetNpcShowFilter / SetDoodadShowFilter / SetHouseShowFilter(on) for the three folders (Merchants, Infrastructure,
-- Housing Province: the lists of map/locale/ko.lua). An icon of a folder shows when the folder and its own check are on.
local FOLDERS = {
  npc = {
    "npc_store_weapon", "npc_store_defense", "npc_store_blacksmith", "npc_store_material", "npc_store_furniture",
    "npc_store_tree", "npc_store_plants", "npc_store_livestock", "npc_store_roamer", "npc_stabler", "npc_store_bank",
    "npc_store_auction_house", "npc_store_expedition", "npc_store_pirate_expedition", "npc_ability_changer",
    "npc_store_trade", "npc_store_delphinad", "npc_store_enlisting", "npc_store_gladiator",
  },
  doodad = {
    "doodad_mail", "doodad_inn", "doodad_cook", "doodad_craft", "doodad_archium", "doodad_alchemy", "doodad_metal",
    "doodad_timber", "doodad_fabric", "doodad_leather", "doodad_stone", "doodad_coatofarms", "doodad_woodwork",
    "doodad_paper", "doodad_print", "doodad_art_work", "doodad_fish_stand", "doodad_weapon", "doodad_light_armor",
    "doodad_leather_armor", "doodad_heavy_armor", "doodad_special_product", "doodad_portal",
    "doodad_portal_archemall", "doodad_portal_dungeon",
  },
  house = {
    "house_normal_house", "house_sea_farm_house", "house_high_house", "house_pumpkin_house", "house_farm_house",
    "house_benny_house", "house_bungalow_house",
  },
}
-- the checks that have no folder
local SINGLES = { "corpse_pos", "npc_station", "npc_graveyard", "common_farm", "housing_house", "housing_farm", "housing_fishfarm" }

local FOLDER_OF = {}
for folder, kinds in pairs(FOLDERS) do
  for _, kind in ipairs(kinds) do FOLDER_OF[kind] = folder end
end

local folderOn = { npc = true, doodad = true, house = true }
local checked = {} -- kind -> bool, as the window last said
local filterVersion = 0

-- icon type (the M_* numbers map/common.lua makes) -> kind; known once the map scripts have run
local kindOfType
local function kindOf(iconType)
  if kindOfType == nil then
    if M_NPC_STATION == nil then return nil end
    kindOfType = {}
    local function add(kind)
      local n = _G["M_" .. string.upper(kind)]
      if n ~= nil then kindOfType[n] = kind end
    end
    for kind in pairs(FOLDER_OF) do add(kind) end
    for _, kind in ipairs(SINGLES) do add(kind) end
  end
  return kindOfType[iconType]
end

X2Map.SetNpcFilter = function(self, iconType, on)
  local kind = kindOf(iconType)
  if kind ~= nil and checked[kind] ~= (on ~= false) then
    checked[kind] = on ~= false
    filterVersion = filterVersion + 1
  end
end
local function setFolder(folder, on)
  if folderOn[folder] ~= (on ~= false) then
    folderOn[folder] = on ~= false
    filterVersion = filterVersion + 1
  end
end
X2Map.SetNpcShowFilter = function(self, on) setFolder("npc", on) end
X2Map.SetDoodadShowFilter = function(self, on) setFolder("doodad", on) end
X2Map.SetHouseShowFilter = function(self, on) setFolder("house", on) end

local function shown(kind)
  local folder = FOLDER_OF[kind]
  if folder ~= nil and not folderOn[folder] then return false end
  return checked[kind] ~= false
end

-- The quest notifier's map decals ----------------------------------------------------------------------------------
-- questcontext/quest_context_notifier.lua's AddMapQuestSign(qType) asks the map for a decal index and hands it over:
-- `X2Map:AddNotifyQuestInfo(decalIndex, qType, show)` (line 23 of AddMapQuestSign; Update line 38, Remove line 27).
-- The tables it draws with were given by map/common.lua: `X2Map:SetNotifyCoords` (the tracker's coloured 1..9
-- badges, lines 1873-1876), `SetNotifyAreaCoords` (the map's grey 1..9 numbers, lines 1939-1944) and
-- `SetNotifyAreaColors` (the matching area colours, alpha AREA_COLOR_ALPHA, lines 2009-2014).
local notifyCoords, areaCoords, areaColors = {}, {}, {}
local decals = {} -- qType -> decal index (1..9), as the notifier set it
-- The natives take the count first (`X2Map:SetNotifyCoords(count, coords)`, common.lua lines 1873-1876).
X2Map.SetNotifyCoords = function(self, count, coords) notifyCoords = type(coords) == "table" and coords or {} end
X2Map.SetNotifyAreaCoords = function(self, count, coords) areaCoords = type(coords) == "table" and coords or {} end
X2Map.SetNotifyAreaColors = function(self, count, colors) areaColors = type(colors) == "table" and colors or {} end
local function setDecal(index, qtype, show)
  index, qtype = tonumber(index), tonumber(qtype)
  if index == nil or qtype == nil then return end
  if show == false or index == 0 then decals[qtype] = nil else decals[qtype] = index end
end
X2Map.AddNotifyQuestInfo = function(self, index, qtype, show) setDecal(index, qtype, show) end
X2Map.UpdateNotifyQuestInfo = function(self, index, qtype, show) setDecal(index, qtype, show) end
X2Map.RemoveNotifyQuestInfo = function(self, qtype) decals[tonumber(qtype)] = nil end
X2Map.RemoveAllNotifyQuestInfo = function(self) decals = {} end

-- The client tables are passed 1-based (common.lua), while the native decal index runs 1..9 over the entries
-- 2..10 (the first entry belongs to decal 0). The notifier's own `dingbetCoords` spells the same 0..9 indexing.
local function decalEntry(list, index) return list and list[index + 1] end

-- The fallback copies of common.lua's tables (their setter runs while the world stage loads).
local DEFAULT_AREA_COORDS = {
  { 480, 72, 24, 24 },
  { 240, 0, 16, 16 }, { 256, 0, 16, 16 }, { 272, 0, 16, 16 },
  { 240, 16, 16, 16 }, { 256, 16, 16, 16 }, { 272, 16, 16, 16 },
  { 240, 32, 16, 16 }, { 256, 32, 16, 16 }, { 272, 32, 16, 16 },
}
local DEFAULT_AREA_COLORS = {
  { 40, 148, 63, 0.5 }, { 194, 96, 107, 0.5 }, { 211, 163, 60, 0.5 }, { 138, 166, 40, 0.5 },
  { 210, 106, 29, 0.5 }, { 40, 148, 63, 0.5 }, { 55, 95, 223, 0.5 }, { 73, 167, 211, 0.5 },
  { 225, 150, 219, 0.5 }, { 142, 76, 203, 0.5 },
}

-- The decal index of a quest: what the notifier recorded (or its own GetDecalState), else (a quest list restored
-- before the notifier ran) its place in the tracker.
local function decalOf(qtype)
  local index = decals[qtype]
  if index ~= nil then return index end
  if type(GetDecalState) == "function" then
    local ok, value = pcall(GetDecalState, qtype)
    if ok and value ~= nil then return value end
  end
  if X2Quest ~= nil and X2Quest.GetActiveQuestListCount ~= nil then
    local count = X2Quest:GetActiveQuestListCount()
    for i = 1, math.min(count, 9) do
      if X2Quest:GetActiveQuestType(i) == qtype then return i end
    end
  end
  return nil
end

local function decalKey()
  local parts = { HUD.map_quest_areas_version or -1 }
  if X2Quest ~= nil and X2Quest.GetActiveQuestListCount ~= nil then
    local count = X2Quest:GetActiveQuestListCount()
    for i = 1, count do
      local qtype = X2Quest:GetActiveQuestType(i)
      parts[#parts + 1] = qtype or 0
      parts[#parts + 1] = decalOf(qtype) or 0
    end
  end
  return table.concat(parts, ":")
end

-- The roadmap's "NPC" check: the dots of the units in view are only worth computing while it is on.
-- The active-character type-5 bindings replace these fallbacks after this script loads.
local roadmapOption
X2.GetRoadmapOptionUiData = function(self) return roadmapOption end
X2.SetRoadmapOptionUiData = function(self, data)
  roadmapOption = data
  HUD.map_dots(data ~= nil and data.npcShow == true)
end

-- The places ---------------------------------------------------------------------------------------------------
-- The place on a road map image: the roadmap window (926 x 556) is the map rectangle, the image sits in it at its
-- own frame (see ingame.lua, AAUI.RoadMap).
local function onRoadImage(level, id, formKey, uw, vw)
  local f = HUD_ROAD_FRAMES[level] and HUD_ROAD_FRAMES[level][id]
  local form = FORM_ROAD_MAP_LIST and FORM_ROAD_MAP_LIST[formKey]
  if f == nil and form ~= nil then f = { form[1], form[2], form[5], form[6] } end
  if f == nil then return uw, vw end
  return (uw * 926 - f[1]) / f[3], (vw * 556 - f[2]) / f[4]
end

-- the tooltip of an icon: the unit's or doodad's name, else the symbol's text (the filter window's label)
local symbolTexts = {}
local function tipOf(ic)
  if ic.tip ~= "" then return ic.tip end
  local text = symbolTexts[ic.kind]
  if text == nil then
    local ok, t = pcall(function() return locale.map.getSymbolText(X2Map:GetMapIconText(_G["M_" .. string.upper(ic.kind)])) end)
    text = (ok and type(t) == "string") and t or false
    symbolTexts[ic.kind] = text
  end
  return text or nil
end

local function inside(u, v)
  return u >= 0 and u <= 1 and v >= 0 and v <= 1
end

-- the markers of the roadmap's map (level 2 the zone, 3 a city) and of the big map, rebuilt when the list, the
-- filters or the map change
local road = { key = nil, list = {}, version = 0 }
local world = { key = nil, list = {}, version = 0 }

-- The objective circles: a translucent ring in the notifier's colour (`SetNotifyAreaColors`) and, at its centre,
-- the grey number cell of `SetNotifyAreaCoords`. `w`/`h` are fractions of the map drawable so they scale with the
-- roadmap size and the big map's expansion; the circle is on the background layer so the icons stay above it.
local AREA_ICON = "ui/map/territory_circle.dds"
local ICON_SHEET = "ui/map/icon/npc_icon.dds"

local function areaColor(index)
  local c = decalEntry(areaColors, index) or DEFAULT_AREA_COLORS[index + 1]
  return { (c[1] or 0) / 255, (c[2] or 0) / 255, (c[3] or 0) / 255, c[4] or 0.5 }
end

local function addArea(out, level, id, a, rect, u, v, index)
  out[#out + 1] = {
    level = level, id = id, u = u, v = v, icon = AREA_ICON,
    w = 2 * a.radius / rect[3], h = 2 * a.radius / rect[4],
    color = areaColor(index), layer = "background",
  }
  local cell = decalEntry(areaCoords, index) or DEFAULT_AREA_COORDS[index + 1]
  if cell ~= nil then
    out[#out + 1] = { level = level, id = id, u = u, v = v, icon = ICON_SHEET, coords = cell }
  end
end

-- The big map: the circle on its zone map and, when it is in a city, on the city map.
local function worldAreas(out)
  local zones, cities = HUD.map_zone_rects or {}, HUD.map_city_rects or {}
  for _, a in ipairs(HUD.map_quest_areas or {}) do
    local index = decalOf(a.qtype)
    if index ~= nil and index >= 1 and index <= 9 then
      local zr = zones[a.zone]
      if zr ~= nil and zr[3] > 0 then
        local u, v = (a.x - zr[1]) / zr[3], 1 - (a.y - zr[2]) / zr[4]
        if inside(u, v) then addArea(out, 2, a.zone, a, zr, u, v, index) end
      end
      local cr = a.city and cities[a.city]
      if cr ~= nil and cr[3] > 0 then
        local u, v = (a.x - cr[1]) / cr[3], 1 - (a.y - cr[2]) / cr[4]
        if inside(u, v) then addArea(out, 3, a.city, a, cr, u, v, index) end
      end
    end
  end
end

-- The roadmap: only the map it shows.
local function roadAreas(out, r)
  local s = HUD.state
  local zones, cities = HUD.map_zone_rects or {}, HUD.map_city_rects or {}
  for _, a in ipairs(HUD.map_quest_areas or {}) do
    local index = decalOf(a.qtype)
    if index ~= nil and index >= 1 and index <= 9 then
      if r.level == 3 then
        local cr = a.city and a.city == r.zone and cities[a.city]
        if cr and cr[3] > 0 then
          local u, v = onRoadImage(3, r.zone, "", (a.x - cr[1]) / cr[3], 1 - (a.y - cr[2]) / cr[4])
          if inside(u, v) then addArea(out, 3, a.city, a, cr, u, v, index) end
        end
      elseif a.zone == r.zone then
        local zr = zones[a.zone]
        if zr ~= nil and zr[3] > 0 then
          local key = string.upper(s.zone_key_name) .. "_100"
          local u, v = onRoadImage(2, r.zone, key, (a.x - zr[1]) / zr[3], 1 - (a.y - zr[2]) / zr[4])
          if inside(u, v) then addArea(out, 2, a.zone, a, zr, u, v, index) end
        end
      end
    end
  end
end

local function roadMarkers(r)
  local s = HUD.state
  local key = table.concat({ HUD.map_icons_version or -1, filterVersion, r.level, r.zone, s.zone_group, decalKey() }, ":")
  if road.key == key then return road.list, road.version end
  road.key, road.version, road.list = key, road.version + 1, {}
  local zones, cities = HUD.map_zone_rects or {}, HUD.map_city_rects or {}
  for _, ic in ipairs(HUD.map_icons or {}) do
    if shown(ic.kind) then
      local u, v
      if r.level == 3 then
        local cr = ic.city and ic.city == r.zone and cities[ic.city]
        if cr and cr[3] > 0 then
          u, v = onRoadImage(3, r.zone, "", (ic.x - cr[1]) / cr[3], 1 - (ic.y - cr[2]) / cr[4])
        end
      elseif ic.zone == r.zone then
        local zr = zones[ic.zone] or (s.zone_group == ic.zone and s.zone_rect)
        if zr ~= nil and zr[3] > 0 then
          u, v = onRoadImage(2, r.zone, string.upper(s.zone_key_name) .. "_100", (ic.x - zr[1]) / zr[3], 1 - (ic.y - zr[2]) / zr[4])
        end
      end
      if u ~= nil and inside(u, v) then
        road.list[#road.list + 1] = { u = u, v = v, kind = ic.kind, tooltip = tipOf(ic) }
      end
    end
  end
  roadAreas(road.list, r)
  return road.list, road.version
end

local function worldMarkers()
  local s = HUD.state
  local key = table.concat({ HUD.map_icons_version or -1, filterVersion, s.zone_group, decalKey() }, ":")
  if world.key == key then return world.list, world.version end
  world.key, world.version, world.list = key, world.version + 1, {}
  local zones, cities = HUD.map_zone_rects or {}, HUD.map_city_rects or {}
  for _, ic in ipairs(HUD.map_icons or {}) do
    if shown(ic.kind) then
      -- the whole map image covers the map rectangle: the place is the plain fraction of it
      local zr = zones[ic.zone] or (s.zone_group == ic.zone and s.zone_rect)
      if zr ~= nil and zr[3] > 0 then
        local u, v = (ic.x - zr[1]) / zr[3], 1 - (ic.y - zr[2]) / zr[4]
        if inside(u, v) then
          world.list[#world.list + 1] = { level = 2, id = ic.zone, u = u, v = v, kind = ic.kind, tooltip = tipOf(ic) }
        end
      end
      local c = ic.city and cities[ic.city]
      if c ~= nil and c[3] > 0 then
        local u, v = (ic.x - c[1]) / c[3], 1 - (ic.y - c[2]) / c[4]
        if inside(u, v) then
          world.list[#world.list + 1] = { level = 3, id = ic.city, u = u, v = v, kind = ic.kind, tooltip = tipOf(ic) }
        end
      end
    end
  end
  worldAreas(world.list)
  return world.list, world.version
end

local roadMapHook, worldMapHook = AAUI.RoadMap, AAUI.WorldMap
AAUI.RoadMap = function()
  local r = roadMapHook()
  if r ~= nil then r.markers, r.markers_version = roadMarkers(r) end
  return r
end
AAUI.WorldMap = function()
  local r = worldMapHook()
  if r ~= nil then r.markers, r.markers_version = worldMarkers() end
  return r
end
