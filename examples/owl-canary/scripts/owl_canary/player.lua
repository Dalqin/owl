-- Runs once a game is loaded. Shows a message on screen a few seconds in,
-- and logs a line, so you can see the mod working in the game itself.
local ui = require('openmw.ui')

local shown = false
local elapsed = 0

return {
    engineHandlers = {
        onUpdate = function(dt)
            if shown then return end
            elapsed = elapsed + dt
            if elapsed > 3 then
                shown = true
                ui.showMessage('owl canary: this mod is loaded')
                print('owl canary: player script ran')
            end
        end,
    },
}
