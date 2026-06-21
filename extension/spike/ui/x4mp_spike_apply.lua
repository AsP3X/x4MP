-- Human: Q2 apply (M0.75 spike) — drive a proxy ship's position via the real X4 FFI.
-- Human: MD spawns the proxy and ticks us; we read the player ship's sector position,
-- Human: delay it ~1s through a ring buffer, and write it (offset +4km) onto the proxy.
-- Agent: USES C.GetObjectPositionInSector / C.SetObjectSectorPos (the MD action
-- Agent: 'set_object_position' does NOT exist — that was the Q2 blocker). One-directional
-- Agent: MD->Lua only (RegisterEvent). DIAGNOSTICS go out the SirNuke pipe to the server log
-- Agent: because debuglog.txt does NOT capture our DebugError in this launch config.
-- Agent: LISTENS x4mp.spike.apply.bind (proxy component) and x4mp.spike.apply.tick (10Hz).

local ffi = require("ffi")
local C = ffi.C

-- Agent: proxy_id/sector_id are UniverseIDs; buf is the delay ring of sector positions.
local proxy_id = nil
local sector_id = nil
local buf = {}
local DELAY_TICKS = 10      -- ~1s at the MD 10Hz tick
local OFFSET_X = 4000.0     -- meters in sector frame, so the proxy is visibly separate
local tick_count = 0
local applied = 0

-- Agent: Emit diagnostics through the SHARED pipe helper (x4mp_spike_pipe.lua), which calls
-- Agent: Connect_Pipe before writing — my previous version skipped Connect_Pipe so every
-- Agent: Schedule_Write silently dropped. Helper is loaded before us by ui.xml (_G global).
-- Agent: We lazy-connect once (after sn_mod_support_apis is ready) on the first diag.
local pipe_inited = false

-- Human: Send one diagnostic line to the server via the bridge pipe (and DebugError as backup).
-- Agent: EMITS spike.apply.diag {msg=...}; pcall-guarded so it can never break the apply loop.
local function diag(msg)
  DebugError("[X4MP spike] " .. msg)
  local helper = _G.x4mp_spike_pipe
  if helper then
    if not pipe_inited then
      pcall(helper.init_pipe)
      pipe_inited = true
    end
    local line = '{"type":"spike.apply.diag","msg":"' .. tostring(msg):gsub('"', "'") .. '"}'
    pcall(helper.write_line, line)
  end
end

-- Human: Bind to the proxy spawned by MD. Param is the MD component reference (as a string).
-- Agent: CONVERTS component->UniverseID via ConvertStringTo64Bit; RESOLVES its sector. All FFI
-- Agent: in pcall so a missing declaration is reported to the server log, not swallowed.
function OnSpikeApplyBind(_, param)
  buf = {}
  applied = 0
  tick_count = 0
  proxy_id = nil
  sector_id = nil

  local ok, err = pcall(function()
    proxy_id = ConvertStringTo64Bit(tostring(param))
    if proxy_id and proxy_id ~= 0 then
      sector_id = C.GetContextByClass(proxy_id, "sector", false)
    end
  end)
  if ok then
    diag("bind ok proxy=" .. tostring(proxy_id) .. " sector=" .. tostring(sector_id) .. " param=" .. tostring(param))
  else
    diag("bind FFI ERROR: " .. tostring(err) .. " param=" .. tostring(param))
  end
end

-- Human: Per-tick: sample the player ship, replay the delayed sample onto the proxy.
-- Agent: READS C.GetObjectPositionInSector(player); APPLIES C.SetObjectSectorPos(proxy).
function OnSpikeApplyTick()
  if not proxy_id or proxy_id == 0 or not sector_id or sector_id == 0 then
    -- First few bad ticks are expected before bind; report once.
    if tick_count == 0 then
      diag("tick skipped: no proxy/sector bound yet (proxy=" .. tostring(proxy_id) .. " sector=" .. tostring(sector_id) .. ")")
      tick_count = 1
    end
    return
  end

  local ok, err = pcall(function()
    -- Source ship the player is currently flying (fallback to the player object).
    local src = C.GetPlayerOccupiedShipID()
    if src == nil or src == 0 then
      src = C.GetPlayerObjectID()
    end
    if src == nil or src == 0 then
      return
    end

    local p = C.GetObjectPositionInSector(src)
    buf[#buf + 1] = { x = p.x, y = p.y, z = p.z, yaw = p.yaw, pitch = p.pitch, roll = p.roll }

    if #buf > DELAY_TICKS then
      local t = table.remove(buf, 1)
      -- Agent: ffi.new builds a UIPosRot by value for the C call (offset +X so it sits beside us).
      local pr = ffi.new("UIPosRot", { x = t.x + OFFSET_X, y = t.y, z = t.z, yaw = t.yaw, pitch = t.pitch, roll = t.roll })
      C.SetObjectSectorPos(proxy_id, sector_id, pr)
      applied = applied + 1

      -- Read back every ~1s to PROVE the set took effect (read-back should match what we wrote).
      tick_count = tick_count + 1
      if (applied % 10) == 1 then
        local rb = C.GetObjectPositionInSector(proxy_id)
        diag(string.format("apply #%d set x=%.1f z=%.1f -> readback x=%.1f z=%.1f",
          applied, t.x + OFFSET_X, t.z, rb.x, rb.z))
      end
    end
  end)
  if not ok then
    diag("tick FFI ERROR: " .. tostring(err))
  end
end

-- Human: Stop = clear local state (MD stops ticking when its flag is off).
function OnSpikeApplyStop()
  diag("apply stop after " .. tostring(applied) .. " applies")
  buf = {}
end

RegisterEvent("x4mp.spike.apply.bind", OnSpikeApplyBind)
RegisterEvent("x4mp.spike.apply.tick", OnSpikeApplyTick)
RegisterEvent("x4mp.spike.apply.stop", OnSpikeApplyStop)

diag("apply Lua loaded (FFI SetObjectSectorPos, pipe diagnostics)")
