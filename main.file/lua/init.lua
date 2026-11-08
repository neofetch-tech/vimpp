-- Startup extension entry point for Vim++.
vimpp = vimpp or {}
-- Lua extensions may consume keys and report a message in the status line.
vimpp.on_key = function(key)
    if key == "F2" then
        vimpp.notify("Lua extension received F2")
        return true
    end
    return false
end
