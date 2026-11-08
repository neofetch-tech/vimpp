-- Vim++ Lua API bootstrap. Pure Lua on purpose: the Rust side only reads
-- these tables, so the hot path (key dispatch) never allocates across FFI.

vimpp = vimpp or {}

-- Load-time outbox: scripts push; Rust drains after the script returns.
vimpp._out = { log = {}, options = {}, commands = {} }
vimpp._keymaps = vimpp._keymaps or {}
vimpp._commands = vimpp._commands or {}
vimpp.keymap = { set = function(mode, lhs, rhs)
  local mode_ = tostring(mode)
  if mode_ == "" or mode_ == nil then mode_ = "n" end
  vimpp._keymaps[mode_ .. "|" .. tostring(lhs)] = rhs
end }

--- Echo during load; queued and shown on the command line afterwards.
function vimpp.echo(msg)
  table.insert(vimpp._out.log, tostring(msg))
end

--- Buffer an option until the load finishes: set_option("number", true).
function vimpp.set_option(k, v)
  table.insert(vimpp._out.options, tostring(k) .. "=" .. tostring(v))
end

--- Neovim-flavoured keymaps: rhs may be a string ("<Cmd>w<CR>") or a function.
-- (The table is created above with its `set` — this alias documents the API.)

--- Register a user ex command: vimpp.command("Name", function(args) ... end).
function vimpp.command(name, fn)
  vimpp._commands[tostring(name)] = fn
end

--- Run an ex command as soon as the current script finishes loading.
function vimpp.exec(cmd)
  table.insert(vimpp._out.commands, tostring(cmd))
end

-- Neovim compatibility shim: enough `vim.*` for maps, commands and opts.
vim = vimpp
vim.cmd = vimpp.exec
vim.echo = vimpp.echo
vim.api = vim.api or {}
vim.api.nvim_set_keymap = function(mode, lhs, rhs)
  vimpp.keymap.set(mode, lhs, rhs)
end
vim.api.nvim_command = vimpp.exec
vim.g = setmetatable({}, {
  __newindex = function() end,
  __index = function() return nil end,
})
vim.o = setmetatable({}, {
  __newindex = function(_, k, v) vimpp.set_option(k, v) end,
  __index = function() return nil end,
})

-- Errors surface as '!lua error: ...' on the command line instead of crashing.
return function(err)
  __vimpp_err = tostring(err)
end
