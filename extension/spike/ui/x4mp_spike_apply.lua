-- Human: Q2 apply — drive a proxy ship from inbound spike samples (snap + lerp modes).
-- Agent: READS pipe NDJSON; APPLIES position/rotation to $SpikeProxyShip via MD cues.
local pipe = require("extensions.x4mp_spike.ui.x4mp_spike_pipe")

local MODE_SNAP = "snap"
local MODE_LERP = "lerp"
local mode = MODE_LERP
local running = false
local proxy_ship = nil
local last_sample = nil
local next_sample = nil
local lerp_t = 0
local lerp_duration_s = 0.1
local samples_applied = 0

local function parse_sample(line)
  if not (json and json.decode) then
    return nil
  end
  local ok, sample = pcall(json.decode, line)
  if ok then
    return sample
  end
  return nil
end

-- Human: Apply sample directly to proxy (snap mode).
-- Agent: RAISES MD event with target position for create_ship proxy.
local function apply_snap(sample)
  if not sample or not sample.position then
    return
  end
  samples_applied = samples_applied + 1
  RaiseNPCEvent("x4mp.spike.apply.position", {
    ship = proxy_ship,
    x = sample.position.x,
    y = sample.position.y,
    z = sample.position.z,
    yaw = sample.rotation and sample.rotation.yaw or 0,
    mode = MODE_SNAP,
  })
end

-- Human: Interpolate between last and next sample (smooth mode).
-- Agent: APPLIES lerp position each tick until next sample arrives.
local function apply_lerp_step(dt)
  if not last_sample or not next_sample then
    return
  end
  lerp_t = lerp_t + dt
  local alpha = math.min(1, lerp_t / lerp_duration_s)
  local a = last_sample.position
  local b = next_sample.position
  if not a or not b then
    return
  end
  local x = a.x + (b.x - a.x) * alpha
  local y = a.y + (b.y - a.y) * alpha
  local z = a.z + (b.z - a.z) * alpha
  RaiseNPCEvent("x4mp.spike.apply.position", {
    ship = proxy_ship,
    x = x,
    y = y,
    z = z,
    yaw = next_sample.rotation and next_sample.rotation.yaw or 0,
    mode = MODE_LERP,
  })
  if alpha >= 1 then
    last_sample = next_sample
    next_sample = nil
    lerp_t = 0
  end
end

local function ingest_sample(sample)
  if mode == MODE_SNAP then
    apply_snap(sample)
    return
  end
  if not last_sample then
    last_sample = sample
    apply_snap(sample)
    return
  end
  next_sample = sample
  lerp_t = 0
end

-- Human: Poll pipe for one sample line per tick.
-- Agent: READS x4mp_spike_pipe.read_line; APPLIES via snap or lerp.
function OnSpikeApplyTick()
  if not running then
    return
  end
  local line = pipe.read_line()
  if line then
    local sample = parse_sample(line)
    if sample and sample.type == "spike.ship_sample" then
      ingest_sample(sample)
    end
  elseif mode == MODE_LERP then
    apply_lerp_step(0.1)
  end
end

function StartSpikeApply(ship_id, apply_mode)
  pipe.init_pipe()
  proxy_ship = ship_id
  mode = apply_mode or MODE_LERP
  running = true
  samples_applied = 0
  last_sample = nil
  next_sample = nil
  DebugInfo("[X4MP spike] apply started mode=" .. tostring(mode))
end

function StopSpikeApply()
  running = false
  DebugInfo("[X4MP spike] apply stopped after " .. tostring(samples_applied) .. " samples")
end

RegisterEvent("x4mp.spike.apply.start", StartSpikeApply)
RegisterEvent("x4mp.spike.apply.stop", StopSpikeApply)
RegisterEvent("x4mp.spike.apply.tick", OnSpikeApplyTick)

DebugInfo("[X4MP spike] apply module loaded")
