-- Human: Q1 capture — sample player ship state at ~5-10 Hz and emit NDJSON.
-- Agent: READS GetPlayerPrimaryShipID + GetComponentData; EMITS spike.ship_sample lines.
local pipe = require("extensions.x4mp_spike.ui.x4mp_spike_pipe")

local INTERVAL_S = 0.1
local running = false
local sample_count = 0
local last_emit_ms = 0

local function now_ms()
  if GetCurRealTime then
    return GetCurRealTime() * 1000
  end
  return sample_count * (INTERVAL_S * 1000)
end

-- Human: Build one capture sample table from the player-controlled ship.
-- Agent: READS position/rotation/sector/speed; RETURNS table or nil if no ship.
local function capture_player_ship()
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

  local yaw, pitch, roll = 0, 0, 0
  if GetComponentData then
    yaw, pitch, roll = GetComponentData(ship, "rotation")
  end

  local sector = "unknown"
  if GetComponentData then
    sector = GetComponentData(ship, "sector") or sector
  end

  local speed = 0
  if GetComponentData then
    speed = GetComponentData(ship, "speed") or 0
  end

  return {
    type = "spike.ship_sample",
    t_ms = now_ms(),
    ship_id = tostring(ship),
    sector = tostring(sector),
    position = { x = x, y = y, z = z },
    rotation = { yaw = yaw, pitch = pitch, roll = roll },
    speed = speed,
  }
end

-- Human: Serialize and emit one sample over the pipe.
-- Agent: EMITS NDJSON via x4mp_spike_pipe.write_line.
local function emit_sample()
  local sample = capture_player_ship()
  if not sample then
    return
  end
  sample_count = sample_count + 1
  sample.seq = sample_count
  if json and json.encode then
    pipe.write_line(json.encode(sample))
  else
    pipe.write_line(string.format(
      '{"type":"spike.ship_sample","seq":%d,"t_ms":%.0f}',
      sample_count,
      sample.t_ms
    ))
  end
  last_emit_ms = sample.t_ms
end

-- Human: Frame/tick handler — gate emits to INTERVAL_S cadence.
-- Agent: CALLS emit_sample when interval elapsed; REGISTER via MD or SetScript.
function OnSpikeCaptureTick()
  if not running then
    return
  end
  local t = now_ms()
  if t - last_emit_ms >= (INTERVAL_S * 1000) then
    emit_sample()
  end
end

function StartSpikeCapture()
  pipe.init_pipe()
  running = true
  sample_count = 0
  last_emit_ms = 0
  DebugInfo("[X4MP spike] capture started (interval " .. tostring(INTERVAL_S) .. "s)")
end

function StopSpikeCapture()
  running = false
  DebugInfo("[X4MP spike] capture stopped after " .. tostring(sample_count) .. " samples")
end

RegisterEvent("x4mp.spike.capture.start", StartSpikeCapture)
RegisterEvent("x4mp.spike.capture.stop", StopSpikeCapture)
RegisterEvent("x4mp.spike.capture.tick", OnSpikeCaptureTick)

DebugInfo("[X4MP spike] capture module loaded")
