-- rex-flash: flash.nvim-style search over the focused pane's visible text.
-- Append this file to your Rex config (~/.config/rex/init.lua). Change the key
-- on the last line if ctrl+b s is taken.
--
-- The picker runs in a floating terminal laid exactly over the pane, at its
-- size in cells, so its frame lines up with the text it captured. The float
-- runs on the pane's host, so a pane on another Rex host needs rex-flash
-- installed on that host too.
do
  local terminal = "com.superlogical.terminal"

  -- The float runs on the pane's host with a minimal environment: cover where
  -- cargo, Homebrew and Nix put rex-flash and rex.
  local script = [[
export PATH="$REX_BIN_DIR:$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:/etc/profiles/per-user/$USER/bin:$HOME/.nix-profile/bin:/run/current-system/sw/bin:$PATH"
if command -v rex-flash >/dev/null; then
  rex-flash --block "$1" --cols "$2" --rows "$3" ${4:+--screen "$4"} || read -r _
else
  echo "rex-flash is not installed on this host"; read -r _
fi
rex focus "$1" 2>/dev/null
]]

  -- Tiled panes in layout order.
  local function leaves(node, out)
    if node.block_id then
      out[#out + 1] = node
    else
      leaves(node.split.before, out)
      leaves(node.split.after, out)
    end
    return out
  end

  -- Where a block sits in its window, in window fractions. A zoomed block fills
  -- the window; a floating layer's layout is relative to the layer's bounds.
  local function block_bounds(win, block_id)
    if win.zoomed_block_id == block_id then return { 0, 0, 1, 1 } end
    for _, l in ipairs(win.layers) do
      for _, p in ipairs(leaves(l.layout, {})) do
        if p.block_id == block_id then
          local b, r = l.bounds, p.rect
          return { b[1] + r[1] * b[3], b[2] + r[2] * b[4], r[3] * b[3], r[4] * b[4] }
        end
      end
    end
  end

  -- The last n rows of a vt capture: the screen, without the scrollback before
  -- it. vt separates rows with \r\n and doesn't end the last one; unlike text, it
  -- keeps the blank rows at the bottom of the screen.
  local function last_rows(text, n)
    local nl = {}
    for i in text:gmatch("()\n") do nl[#nl + 1] = i end
    local cut = nl[#nl - n + 1]
    return cut and text:sub(cut + 1) or text
  end

  rex.action{
    name  = "rex_flash",
    title = "Flash: Search Visible Text…",
    run   = function(ctx)
      local here = assert(rex.session.describe_block{ block_id = ctx.block_id }).block.window_id
      local win
      for _, w in ipairs(assert(rex.session.view{}).windows) do
        if w.window_id == here then win = w end
      end
      local bounds = assert(win and block_bounds(win, ctx.block_id), "rex-flash: pane not on screen")
      local size = assert(rex.block.call(terminal, "size", { block_id = ctx.block_id }))
      -- A local pane's text is an in-process call away, so the picker paints as
      -- soon as its float starts. A remote pane's format call would carry its
      -- whole scrollback over the network, so the float captures it there.
      local screen = ""
      if not ctx.server then
        local f = rex.block.call(terminal, "format", { block_id = ctx.block_id, format = "vt" })
        if f and f.content and not f.id then screen = last_rows(f.content, size.rows) end
      end
      assert(rex.session.new_layer{
        window_id = here,
        bounds    = bounds,
        focus     = true,
        layout    = rex.layout.block{
          flavor  = "com.superlogical.terminal.shell",
          -- Started at the pane's size: a float otherwise starts at 80x24 and
          -- stays blank until the app resizes it.
          options = { shell = "none", size = { columns = size.columns, rows = size.rows }, command = {
            "/bin/sh", "-c", script, "sh", ctx.block_id, tostring(size.columns), tostring(size.rows), screen,
          } },
        },
      })
    end,
  }
  rex.bind("ctrl+b>s", "rex_flash")
end
