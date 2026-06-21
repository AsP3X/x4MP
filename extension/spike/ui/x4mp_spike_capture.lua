-- Human: Q1 capture — sample player ship state at ~10 Hz and hand NDJSON to MD for pipe write.
-- Agent: READS GetPlayerPrimaryShipID + GetComponentData; EMITS AddUITriggeredEvent('x4mp_spike','sample',line).
-- Agent: WRITE PATH = Lua builds JSON -> UI event -> MD md.Named_Pipes.Write (no Lua require of SirNuke).

local INTERVAL_S = 0.1
local running = false
local sample_count = 0
local last_emit_ms = 0

-- Human: Real-time clock in ms; falls back to a synthetic counter if unavailable.
-- Agent: READS GetCurRealTime; RETURNS milliseconds.
local function now_ms()
  if GetCurRealTime then
    return GetCurRealTime() * 1000
  end
  return sample_count * (INTERVAL_S * 1000)
end

-- Human: Minimal JSON string escaping for sector/ship_id text fields.
-- Agent: ESCAPES backslash and double-quote so the NDJSON line stays valid.
local function esc(s)
  s = tostring(s or "")
  s = s:gsub("\\", "\\\\")
  s = s:gsub('"', '\\"')
  return s
end

-- Human: Hand one NDJSON line to MD, which forwards it to the bridge pipe.
-- Agent: CALLS AddUITriggeredEvent; MD cue Spike_Forward_Sample writes via md.Named_Pipes.Write.
local function emit_line(line)
  if AddUITriggeredEvent then
    AddUITriggeredEvent("x4mp_spike", "sample", line)
  end
end

-- Human: Build one capture sample line from the player-controlled ship.
-- Agent: READS position/rotation/sector/speed; RETURNS NDJSON string or nil if no ship.
local function build_sample_line()
  if not GetPlayerPrimaryShipID then
    return nil
  end
  local ship = GetPlayerPrimaryShipID()
  if not ship or ship == 0 then
    return nil
  end

  local x, y, z = 0, 0, 0
  if GetComponentData then
    x, y, z = GetComponentData(ship, "position")
  end
  x = x or 0; y = y or 0; z = z or 0

  local yaw, pitch, roll = 0, 0, 0
  if GetComponentData then
    yaw, pitch, roll = GetComponentData(ship, "rotation")
  end
  yaw = yaw or 0; pitch = pitch or 0; roll = roll or 0

  local sector = "unknown"
  if GetComponentData then
    sector = GetComponentData(ship, "sector") or sector
  end

  local speed = 0
  if GetComponentData then
    speed = GetComponentData(ship, "speed") or 0
  end

  sample_count = sample_count + 1
  return string.format(
    '{"type":"spike.ship_sample","seq":%d,"t_ms":%.0f,"ship_id":"%s","sector":"%s",'
      .. '"position":{"x":%.3f,"y":%.3f,"z":%.3f},'
      .. '"rotation":{"yaw":%.5f,"pitch":%.5f,"roll":%.5f},"speed":%.3f}',
    sample_count, now_ms(), esc(ship), esc(sector),
    x, y, z, yaw, pitch, roll, speed
  )
end

-- Human: Frame/tick handler — gate emits to INTERVAL_S cadence.
-- Agent: CALLS build_sample_line + emit_line when interval elapsed; REGISTER via MD tick cue.
function OnSpikeCaptureTick()
  if not running then
    return
  end
  local t = now_ms()
  if t - last_emit_ms >= (INTERVAL_S * 1000) then
    local line = build_sample_line()
    if line then
      emit_line(line)
      last_emit_ms = t
    end
  end
end

function StartSpikeCapture()
  running = true
  sample_count = 0
  last_emit_ms = 0
  DebugError("[X4MP spike] capture started (interval " .. tostring(INTERVAL_S) .. "s)")
  -- Human: Immediate server-visible event before the first ship sample.
  -- Agent: EMITS spike.capture.started so the server logs activity at once even if no ship yet.
  emit_line(string.format('{"type":"spike.capture.started","t_ms":%.0f}', now_ms()))
  -- Try one sample right away if a ship is available.
  local line = build_sample_line()
  if line then
    emit_line(line)
    last_emit_ms = now_ms()
  end
end

function StopSpikeCapture()
  if running then
    emit_line(string.format(
      '{"type":"spike.capture.stopped","t_ms":%.0f,"samples":%d}',
      now_ms(), sample_count))
  end
  running = false
  DebugError("[X4MP spike] capture stopped after " .. tostring(sample_count) .. " samples")
end

RegisterEvent("x4mp.spike.capture.start", StartSpikeCapture)
RegisterEvent("x4mp.spike.capture.stop", StopSpikeCapture)
RegisterEvent("x4mp.spike.capture.tick", OnSpikeCaptureTick)

DebugError("[X4MP spike] capture module loaded")
