-- Prints every glyph of box 0 with its origin: x from the left edge of the box and y up from the
-- baseline, in scaled points, and its glyph index in the font.
local id_glyph, id_hlist, id_vlist = node.id("glyph"), node.id("hlist"), node.id("vlist")
local id_glue, id_kern, id_rule, id_math = node.id("glue"), node.id("kern"), node.id("rule"), node.id("math")
local out

-- The width of a glue in a box that is stretched or shrunk (`\hss`, as in a centred numerator).
local function glue_width(glue, box)
  local w = glue.width
  if box.glue_sign == 1 and glue.stretch_order == box.glue_order then
    w = w + box.glue_set * glue.stretch
  elseif box.glue_sign == 2 and glue.shrink_order == box.glue_order then
    w = w - box.glue_set * glue.shrink
  end
  return w
end

local walk_h, walk_v

function walk_h(box, x, base)            -- an hlist: its baseline at height `base`
  local head = box.head
  for n in node.traverse(head) do
    local id = n.id
    if id == id_glyph then
      local f = font.getfont(n.font)
      local info = f and f.characters and f.characters[n.char]
      out[#out + 1] = string.format("%d\t%d\t%d\t%d", (info and info.index) or -1, n.char,
        math.floor(x + (n.xoffset or 0) + 0.5), math.floor(base + (n.yoffset or 0) + 0.5))
      x = x + n.width
    elseif id == id_hlist then
      walk_h(n, x, base - n.shift); x = x + n.width
    elseif id == id_vlist then
      walk_v(n, x, base - n.shift + n.height); x = x + n.width
    elseif id == id_glue then x = x + glue_width(n, box)
    elseif id == id_kern then x = x + n.kern
    elseif id == id_rule then x = x + n.width
    elseif id == id_math then x = x + n.surround
    end
  end
end

function walk_v(box, x, top)             -- a vlist: its top edge at height `top`
  local head = box.head
  local y = top
  for n in node.traverse(head) do
    local id = n.id
    if id == id_hlist then
      y = y - n.height; walk_h(n, x + n.shift, y); y = y - n.depth
    elseif id == id_vlist then
      y = y - n.height; walk_v(n, x + n.shift, y + n.height); y = y - n.depth
    elseif id == id_rule then y = y - n.height - n.depth
    elseif id == id_glue then y = y - glue_width(n, box)
    elseif id == id_kern then y = y - n.kern
    end
  end
end

function dump_glyphs(name)
  out = {}
  walk_h(tex.box[0], 0, 0)
  for _, line in ipairs(out) do texio.write_nl("GLYPH\t" .. name .. "\t" .. line) end
end
