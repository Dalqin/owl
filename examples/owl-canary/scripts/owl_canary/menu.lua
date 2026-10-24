-- Runs in OpenMW's menu context, so it starts before any game is loaded.
-- Its only job is to leave a line in openmw.log proving this mod was loaded.
return {
    engineHandlers = {
        onInit = function()
            print('owl canary: menu script loaded')
        end,
    },
}
