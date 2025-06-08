local _print = print; -- RestrictedInfrastructure redefines print and fires securecalls.

function FillLocalizedClassList()
    _print("STUB: FillLocalizedClassList");
end

bit = bit32; --re export library.

function RegisterStaticConstants(table)
    _print("STUB: RegisterStaticConstants");
end

-- TODO: This has to be implemented in the native side
function CreateFrame(strType, strName, oParent, strInherits)
    _print("STUB: CreateFrame", strType, strName, oParent, strInherits);
    local frame = {};
    frame.SetScript = StubFrame_SetScript;
    frame.RegisterEvent = StubFrame_RegisterEvent;
    frame.UnregisterEvent = StubFrame_UnregisterEvent;
    frame.Hide = StubFrame_Hide;
    frame.CreateTexture = StubFrame_CreateTexture;
    return frame;
end

function StubFrame_SetScript(self, strType, func)
    _print("STUB: SetScript", strType);
end

function StubFrame_RegisterEvent(self, strEvent)
    _print("STUB: RegisterEvent", strEvent);
end

function StubFrame_UnregisterEvent(self, strEvent)
    _print("STUB: UnregisterEvent", strEvent);
end

function StubFrame_Hide(self)
    _print("STUB: Hide");
end

function StubFrame_CreateTexture(self, name, layer, template)
    _print("STUB: CreateTexture", name, layer, template);
    local texture = {};
    return texture;
end

function seterrorhandler()
    _print("STUB: seterrorhandler");
end

function GetItemQualityColor(quality)
    _print("STUB: GetItemQualityColor");
    return 1, 1, 1, "ffffffff";
end

function GetInventorySlotInfo(name)
    _print("STUB: GetInventorySlotInfo", name);
    return 0, "", false;
end

function newproxy(proxy)
    _print("STUB: newproxy", proxy);
    -- TODO: Apparently this was a LUA builtin that has been deprecated as of LUA 5.1? Maybe we need a module/lib with mlua?
    local table = {};
    setmetatable(table, {});
    return table;
end

-- TODO: Implement natively and figure out values
function GetExpansionLevel()
    _print("STUB: GetExpansionLevel");
    return 2;
end

-- TODO: Implement natively
function UnitName(unit)
    _print("STUB: UnitName", unit);
    return "UnitName Stub";
end

-- TODO: Implement natively, investigate the lua sandbox workings
function issecure()
    _print("STUB: issecure");
    return true;
end

function BNGetMaxPlayersInConversation()
    _print("STUB: BNGetMaxPlayersInConversation");
    return 10; -- idk, but bnet is irrelevant anyway
end

function GetChatTypeIndex(str)
    _print("STUB: GetChatTypeIndex", str);
    local n = 1;
    for k, v in pairs(ChatTypeInfo) do
        if (k == str) then
            return n;
        end
        n = n + 1;
    end
    return 0;
end

-- Backwards compat lua things
function strlower(str)
    return str:lower();
end

function strupper(str)
    return str:upper()
end

PI = math.pi;
