-- Load the OSC components as one chunk so their original lexical state and
-- mpv-facing registration order remain unchanged.
local source_path = debug.getinfo(1, 'S').source:gsub('^@', '')
local script_dir = mp.get_script_directory() or source_path:match('^(.*)/[^/]+$') or '.'
local components = {
    'mbv_state.lua',
    'mbv_helpers.lua',
    'mbv_render.lua',
    'mbv_options.lua',
    'mbv_lifecycle.lua',
    'mbv_events.lua',
    'mbv_visibility.lua',
    'mbv_intro.lua',
}

local source = {}
for _, name in ipairs(components) do
    local file = assert(io.open(script_dir .. '/' .. name, 'r'))
    source[#source + 1] = file:read('*a')
    file:close()
end

local compile = loadstring or load
local chunk, err = compile(table.concat(source, '\n'), '@mbv.lua')
assert(chunk, err)
chunk()
