-- The original converter takes the picked bag item from X2Cursor; the Rust slot setters
-- remain the authoritative validation (`changeitemlook.alb` lines 109-115, 158-164).
X2ItemLookConverter.SetConvertBaseItemSlotFromPick = function(self)
  return self:SetConvertBaseItemSlotFromBag(X2Cursor:GetCursorPickedBagItemIndex())
end
X2ItemLookConverter.SetConvertLookItemSlotFromPick = function(self)
  return self:SetConvertLookItemSlotFromBag(X2Cursor:GetCursorPickedBagItemIndex())
end
