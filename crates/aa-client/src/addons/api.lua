-- ADDON_API for one addon's sandbox. The host injects __ADDON_HOST (Rust functions) into the
-- environment before this file runs; only the functions below stay reachable from addon code.
local host = __ADDON_HOST
__ADDON_HOST = nil

local tostring, type, select = tostring, type, select
local table_concat = table.concat

ADDON_API = {}
ADDON_API.baseDir = ""
ADDON_API.timers = {}

local function message_of(self, message)
  if message == nil then
    return tostring(self)
  end
  return tostring(message)
end

ADDON_API.Log = {
  Info = function(self, message) host.log_info(message_of(self, message)) end,
  Err = function(self, message) host.log_err(message_of(self, message)) end,
}

function ADDON_API.GetSettings(addonId)
  return host.get_settings(addonId)
end

function ADDON_API.SaveSettings()
  host.save_settings()
end

function ADDON_API.On(event, callback)
  host.on(event, callback)
end

local function pack(...)
  local args = {}
  for i = 1, select('#', ...) do
    args[i] = select(i, ...)
  end
  return args
end

function ADDON_API.DoIn(...)
  local first = select(1, ...)
  if type(first) == "table" and first == ADDON_API then
    host.do_in(select(2, ...), select(3, ...), pack(select(4, ...)))
  else
    host.do_in(select(1, ...), select(2, ...), pack(select(3, ...)))
  end
end

function ADDON_API.Emit(...)
  local first = select(1, ...)
  if type(first) == "table" and first == ADDON_API then
    host.emit(select(2, ...))
  else
    host.emit(select(1, ...))
  end
end

local function create(ty, id, parent)
  return UIParent:CreateWidget(ty, tostring(id), parent or ADDON_API.rootWindow)
end

ADDON_API.Interface = {}

function ADDON_API.Interface:CreateWidget(ty, id, parent)
  return create(ty, id, parent)
end

function ADDON_API.Interface:CreateEmptyWindow(id)
  return UIParent:CreateWidget("window", tostring(id), ADDON_API.rootWindow)
end

function ADDON_API.Interface:CreateWindow(id, title, x, y, tabs)
  local window = UIParent:CreateWidget("window", tostring(id), ADDON_API.rootWindow)
  window:SetExtent(680, 615)
  if x ~= nil then
    window:AddAnchor("TOPLEFT", "UIParent", x, y or 0)
  else
    window:AddAnchor("CENTER", "UIParent", 0, 0)
  end
  if title ~= nil then
    window:SetTitleText(tostring(title))
  end
  if tabs ~= nil then
    window.tabs = tabs
  end
  return window
end

function ADDON_API.Interface:Free(wnd)
  if wnd ~= nil then
    wnd:EnableHidingIsRemove(true)
    wnd:Show(false)
  end
end

print = function(...)
  local parts = {}
  for i = 1, select('#', ...) do
    parts[i] = tostring(select(i, ...))
  end
  host.log_info(table_concat(parts, "\t"))
end
