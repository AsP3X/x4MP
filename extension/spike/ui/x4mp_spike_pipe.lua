-- Human: Minimal pipe I/O for M0.75 spike (SirNuke Named Pipe when available).
-- Agent: WRITES NDJSON lines; READS inbound lines for apply spike; fallback DebugInfo.
local M = {}

local PIPE_NAME = "x4mp_bridge"
local pipe_ready = false

-- Human: Attempt to open the SirNuke pipe; log if unavailable.
-- Agent: CALLS Pipe API when mod present; RETURNS boolean ready state.
function M.init_pipe()
  if Pipe and Pipe.open then
    local ok = pcall(function()
      Pipe.open(PIPE_NAME)
    end)
    pipe_ready = ok
    if ok then
      DebugInfo("[X4MP spike] pipe open: " .. PIPE_NAME)
    else
      DebugInfo("[X4MP spike] pipe open failed — using DebugInfo fallback")
    end
  else
    DebugInfo("[X4MP spike] SirNuke Pipe API not found — using DebugInfo fallback")
    pipe_ready = false
  end
  return pipe_ready
end

-- Human: Write one NDJSON line to the bridge pipe (or log fallback).
-- Agent: EMITS spike sample line; failure mode = DebugInfo only.
function M.write_line(line)
  if pipe_ready and Pipe and Pipe.write then
    local ok = pcall(function()
      Pipe.write(PIPE_NAME, line .. "\n")
    end)
    if ok then
      return true
    end
  end
  DebugInfo("[X4MP spike OUT] " .. line)
  return false
end

-- Human: Non-blocking read of one NDJSON line from pipe (apply spike).
-- Agent: READS Pipe.read when available; RETURNS line or nil.
function M.read_line()
  if pipe_ready and Pipe and Pipe.read then
    local ok, line = pcall(function()
      return Pipe.read(PIPE_NAME)
    end)
    if ok and line and line ~= "" then
      return line
    end
  end
  return nil
end

return M
