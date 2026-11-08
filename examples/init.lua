-- ~/.vimplus/init.lua — Vim++ is extended natively, single process.
-- Handlers and commands RETURN an action: ":cmd", "!echo text", or raw keys.

-- Custom ex command the Rust core does not know about.
vimpp.command("Half", function(args)
  return ("!Half invoked with: %s"):format(tostring(args))
end)

-- Modal helper: <C-t> toggles line numbers like you just don't care.
vimpp.keymap.set("n", "<C-t>", function()
  return ":set nu!"
end)

-- A string RHS works too — full Vim++ semantics, zero glue code.
vimpp.keymap.set("n", "Y", "y$")

-- vim.cmd is queued and applied as soon as init.lua returns.
vim.cmd(":set relativenumber")
vimpp.set_option("tabstop", 2)

vimpp.echo("init.lua loaded — NamASTE ✌")
