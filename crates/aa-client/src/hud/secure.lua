-- Item security locks: the four-button window's two item modes (inventory/sort_bag.alb:281-347)
-- and the equipment-wide requests. The lock state comes from item flag 0x04 and the unlock
-- deadline; HUD.security_state (secure.rs) reports the ITEM_SECURITY_* value of a bag slot.

local LOCK = ITEM_SECURITY_LOCKED or 1
local lockMode, unlockMode = false, false

local function state(slot)
  return HUD.security_state(slot)
end

X2Item.IsInSecurityLockMode = function(self) return lockMode end
X2Item.IsInSecurityUnlockMode = function(self) return unlockMode end
X2Item.EnterSecurityLockMode = function(self)
  lockMode, unlockMode = true, false
end
X2Item.LeaveSecurityLockMode = function(self) lockMode = false end
X2Item.EnterSecurityUnlockMode = function(self)
  lockMode, unlockMode = false, true
end
X2Item.LeaveSecurityUnlockMode = function(self) unlockMode = false end

-- An unlocking item can be locked again (the pending unlock is cancelled), so only a
-- locked item refuses the lock mode. An empty cell is never a target.
X2Item.IsSecurityLockable = function(self, itemSlot)
  local s = state(itemSlot)
  return s ~= nil and s ~= LOCK
end
X2Item.IsSecurityUnlockable = function(self, itemSlot)
  return state(itemSlot) == LOCK
end
-- The unlock dialog prints this divided by 60 (sort_bag.alb:314-330).
X2Item.GetSecurityUnlockDelayTime = function(self) return 600 end

X2Equipment.SecurityLock = function(self) HUD.secure_equipments(true) end
X2Equipment.SecurityUnlock = function(self) HUD.secure_equipments(false) end

-- A bag click while a lock mode is active: consume it and send the single-item request.
-- (drag.lua calls this before picking the item up.)
function HUD.security_click(slot)
  if lockMode then
    if X2Item:IsSecurityLockable(slot) then HUD.secure_item(slot, true) end
    return true
  elseif unlockMode then
    if X2Item:IsSecurityUnlockable(slot) then HUD.secure_item(slot, false) end
    return true
  end
  return false
end
