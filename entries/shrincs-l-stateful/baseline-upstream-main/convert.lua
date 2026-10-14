-- Upstream's witness layout: the whole stateful signature in one Either, plus the root of
-- the stateless half.
--
--   SIG        = Left(((R, counter, [chains; 64]), list![path...], q))
--   OTHER_ROOT = PK.sl
--
-- SimplicityHL takes hex literals of exactly a type's width, so each field goes in as
-- 0x-prefixed hex, as it comes.

local function hex_list(items)
  local out = {}
  for i, item in ipairs(items) do
    out[i] = "0x" .. item
  end
  return table.concat(out, ", ")
end

function convert(sig)
  return {
    SIG = string.format(
      "Left(((0x%s, 0x%s, [%s]), list![%s], %d))",
      sig.r, sig.counter, hex_list(sig.chains), hex_list(sig.path), sig.q
    ),
    OTHER_ROOT = "0x" .. sig.sl_root,
  }
end
