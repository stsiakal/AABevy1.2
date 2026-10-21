-- The original NPC interaction window used as the function menu.
-- `npc_interaction_view.alb` creates the greeting/message, the four quest button
-- groups (`start`, `progress`, `complete`, `talk`) and one `interactionButtons[i]`
-- per locale.npcInteraction.interactions entry. The native client fills and shows
-- them; we populate the same authored widgets from the received skill list.
local wired = false

local function npc()
  return rawget(_G, "npcInteraction")
end

local function wire(n)
  if wired or not n or not n.window then return false end
  wired = true
  if n.closeButton then
    n.closeButton:SetHandler("OnClick", function() HUD.npc_choice_close() end)
  end
  return true
end

-- The 1.2 menu texts for the functions `npc_interaction_view.alb` lists
-- (`locale.npcInteraction.interactions`): 1 Learn Skills, 2 Change Skillsets, 3 Occupation,
-- 4 Shop, 5 Auction, 6 Receive Divine Blessings, 7 Area Movement, 8 Basic Enchant.
local MENU_TEXT = { store = 4, auction = 5, ability_change = 2, priest = 6, look_converter = 8 }

local function quest_buttons(n)
  for _, group in ipairs({ "start", "progress", "complete", "talk" }) do
    local g = n[group]
    if g and g.buttons then
      for _, button in ipairs(g.buttons) do button:Show(false) end
    end
  end
end

function ShowNpcServiceChoice()
  local n = npc()
  if not n or not n.window or not wire(n) then return end
  local choices = HUD.npc_choice()
  local buttons = n.interactionButtons or {}
  for _, button in ipairs(buttons) do button:Show(false) end
  quest_buttons(n)
  for i, choice in ipairs(choices) do
    local button = buttons[i]
    if button then
      local label = choice.label
      if choice.kind == "quest" then
        label = locale.npcInteraction[choice.what] or choice.what
      else
        local index = MENU_TEXT[choice.key]
        if index and locale.npcInteraction.interactions then
          label = locale.npcInteraction.interactions[index] or label
        end
      end
      button:SetText(label or "")
      button:Show(true)
      button:SetHandler("OnClick", function() HUD.npc_choose(i) end)
    end
  end
  if n.message then
    pcall(function() n.message:SetText(locale.npcInteraction.baseText or "") end)
    n.message:Show(true)
  end
  n.window:Show(true)
end

function HideNpcServiceChoice()
  local n = npc()
  if n and n.window then n.window:Show(false) end
end
