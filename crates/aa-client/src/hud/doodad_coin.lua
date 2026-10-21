-- Original `DLG_TASK_PURCHASE_COIN` dialog over a received doodad purchase.
-- `dialoghandler/dialog_handler.alb` F11 (lines 207-228) is the authored decorator; this
-- only supplies its info and the OK/cancel edges (the native client sends CSBuyCoinItem).
function OpenCoinShop()
  local info = HUD.coin_shop()
  if not info then return end
  local wnd = X2DialogManager:RequestTaskDialog(DLG_TASK_PURCHASE_COIN, info, 0)
  if not wnd then return end
  wnd.btnOk:SetHandler("OnClick", function()
    HUD.coin_buy()
    X2DialogManager:OnOK(wnd:GetId())
  end)
  wnd.btnCancel:SetHandler("OnClick", function()
    HUD.coin_close()
    X2DialogManager:OnCancel(wnd:GetId())
  end)
end
