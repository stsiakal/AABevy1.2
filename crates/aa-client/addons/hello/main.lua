-- Example addon: a small window, an update handler, a timer, a chat event and its own settings.
-- Copy this folder into the addons directory and put "hello" in addons.txt.
local name = "Hello"
local version = "1.0.0"
local window
local settings

local function on_update(dt)
  UPDATE_COUNT = (UPDATE_COUNT or 0) + 1
  LAST_DT = dt
end

local function on_chat(channel, senderId, flag, sender, text)
  CHAT_LINES = (CHAT_LINES or 0) + 1
end

local function on_load()
  ONLOAD_RAN = true
  settings = ADDON_API.GetSettings("hello")
  settings.count = (settings.count or 0) + 1
  ADDON_API.SaveSettings()

  ADDON_API.Log:Info("Hello addon loaded (run " .. tostring(settings.count) .. ").")
  ADDON_API.On("UPDATE", on_update)
  ADDON_API.On("CHAT_MESSAGE", on_chat)
  ADDON_API:DoIn(250, function(what)
    TIMER_FIRED = true
    ADDON_API.Log:Info("Hello timer fired: " .. tostring(what))
  end, "hello timer")

  window = ADDON_API.Interface:CreateWindow("hello_window", "Hello Addon", 120, 120)
  window:SetExtent(260, 96)
  local label = ADDON_API.Interface:CreateWidget("textbox", "hello_label", window)
  label:SetText("Hello from an addon!")
  label:AddAnchor("TOPLEFT", window, 14, 14)
end

local function on_unload()
  ADDON_API.Log:Info("Hello addon unloaded.")
  if window ~= nil then
    ADDON_API.Interface:Free(window)
    window = nil
  end
end

return {
  name = name,
  author = "AABevy1.2",
  desc = "Example addon: window, UPDATE, timer, chat event and saved settings.",
  version = version,
  OnLoad = on_load,
  OnUnload = on_unload,
}
