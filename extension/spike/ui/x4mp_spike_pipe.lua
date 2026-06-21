-- Human: Minimal pipe I/O for M0.75 spike (SirNuke Named Pipes API).

-- Agent: WRITES NDJSON via Pipes.Schedule_Write to x4mp_bridge; READS Read_Pipe; fallback DebugInfo.

local M = {}



local PIPE_NAME = "x4mp_bridge"

local pipe_ready = false

local Pipes = nil



-- Human: Lazy-load SirNuke pipe module after sn_mod_support_apis init.

-- Agent: CALLS require on named_pipes.Interface; RETURNS Pipes table or nil.

local function load_pipes_api()

  if Pipes then

    return Pipes

  end

  local ok, mod = pcall(require, "extensions.sn_mod_support_apis.ui.named_pipes.Interface")

  if ok and mod then

    Pipes = mod

  end

  return Pipes

end



-- Human: Connect X4 as client to the bridge-hosted \\.\pipe\x4mp_bridge.

-- Agent: CALLS Pipes.Connect_Pipe; failure mode = DebugInfo-only capture.

function M.init_pipe()

  local pipes = load_pipes_api()

  if not pipes or not pipes.winpipe_loaded then

    DebugInfo("[X4MP spike] SirNuke winpipe not loaded - using DebugInfo fallback")

    pipe_ready = false

    return false

  end



  local ok, err = pcall(pipes.Connect_Pipe, PIPE_NAME)

  pipe_ready = ok

  if ok then

    DebugInfo("[X4MP spike] pipe connected: " .. PIPE_NAME)

  else

    DebugInfo("[X4MP spike] pipe connect failed - is x4mp-bridge running with X4MP_PIPE_NAME?")

    if err then

      DebugInfo("[X4MP spike] connect error: " .. tostring(err))

    end

  end

  return pipe_ready

end



-- Human: Write one NDJSON line to the bridge pipe (or log fallback).

-- Agent: EMITS spike sample via Schedule_Write; bridge must host the named pipe.

function M.write_line(line)

  local pipes = load_pipes_api()

  if pipe_ready and pipes then

    local msg = line

    if not msg:match("\n$") then

      msg = msg .. "\n"

    end

    local ok = pcall(pipes.Schedule_Write, PIPE_NAME, nil, msg)

    if ok then

      return true

    end

  end

  DebugInfo("[X4MP spike OUT] " .. line)

  return false

end



-- Human: Non-blocking read of one NDJSON line from pipe (apply spike).

-- Agent: READS Pipes.Read_Pipe when available; RETURNS line or nil.

function M.read_line()

  local pipes = load_pipes_api()

  if pipe_ready and pipes then

    local ok, message = pcall(pipes.Read_Pipe, PIPE_NAME)

    if ok and message and message ~= "" and message ~= "ERROR" then

      return message

    end

  end

  return nil

end



_G.x4mp_spike_pipe = M

return M

