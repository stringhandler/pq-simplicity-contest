-- upstream's optimize_fors witness layout (optimized/tools/vectors_to_wit.py), with the
-- key and the message removed from HEAD because the program takes those itself.
--
--   IS_STATEFUL = true
--   HEAD        = (key_idx, R, counter)
--   SIG         = the 64 WOTS+C chain values
--   PATH0       = the first authentication node
--   PATH1/PATH2 = the second and third, if there are any
--   PATH        = the rest
--   ROOT_PART   = PK.sl
--
-- The stateless branch is pruned away on a stateful spend, but every witness the program
-- declares still needs a value, so those get zeros.

local function hex(items, from, to)
  local out = {}
  for i = from, to do
    out[#out + 1] = "0x" .. items[i]
  end
  return table.concat(out, ", ")
end

local function zeros(n)
  local out = {}
  for i = 1, n do out[i] = "0" end
  return "[" .. table.concat(out, ", ") .. "]"
end

function convert(sig)
  local path = sig.path
  local fors_part = "(0, " .. zeros(22) .. ")"
  return {
    IS_STATEFUL = "true",
    HEAD = string.format("(%d, 0x%s, 0x%s)", sig.q, sig.r, sig.counter),
    SIG = "[" .. hex(sig.chains, 1, 64) .. "]",
    PATH0 = "0x" .. path[1],
    PATH1 = path[2] and ("Some(0x" .. path[2] .. ")") or "None",
    PATH2 = path[3] and ("Some(0x" .. path[3] .. ")") or "None",
    PATH = "list![" .. hex(path, 4, #path) .. "]",
    ROOT_PART = "0x" .. sig.sl_root,
    R_SL = "0",
    FORS = "[" .. string.rep(fors_part .. ", ", 4) .. fors_part .. "]",
    X0_COUNTER = "0", X0_SIG = zeros(64), X0_PATH = zeros(12),
    X1_COUNTER = "0", X1_SIG = zeros(64), X1_PATH = zeros(12),
    ROOT_PART_SL = "0",
  }
end
