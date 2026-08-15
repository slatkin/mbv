-- Skip Intro overlay
local skip_intro = make_overlay(999, {
    'skip-intro',
}, function(pw, ph)
    if not skip_intro.visible then return nil end

    -- Size: pill height drives everything; 50% larger than base
    local fs = math.max(18, math.floor(ph / 32))
    local bh = fs + 24                        -- vertical padding around text
    local r  = bh / 2                         -- full pill radius
    local bw = math.max(r * 2 + 60, fs * 6)  -- wide enough for "Skip Intro"

    local pad = 20
    -- Default: lower-right corner; when mouse is near the OSC zone: float above OSC
    local bx = pw - pad - bw
    local by
    if skip_intro.mouse_near then
        by = ph - 145 - bh  -- OSC top element is ~132px above ph; add 13px margin
    else
        by = ph - pad - bh
    end

    set_virt_mouse_area(bx, by, bx + bw, by + bh, 'skip-intro')
    mp.enable_key_bindings('skip-intro')

    local ass = assdraw.ass_new()

    -- Solid pill background — OVERLAY palette colour (#3F3F3F = Rgb 63,63,63)
    ass:new_event()
    ass:pos(bx, by)
    ass:an(7)
    ass:append('{\\bord0\\blur0\\1c&H3F3F3F&\\1a&H00&}')
    ass:draw_start()
    ass:round_rect_cw(0, 0, bw, bh, r, r)
    ass:draw_stop()

    -- "Skip Intro" centred in the pill
    ass:new_event()
    ass:pos(bx + bw / 2, by + bh / 2)
    ass:an(5)  -- centre-centre
    ass:append(string.format('{\\fs%d\\bord0\\blur0\\1c&HFAFAFA&\\bold1}', fs))
    ass:append('Skip Intro')

    return ass.text
end)

skip_intro.end_secs   = 0
skip_intro.mouse_near = false  -- true when mouse is in the OSC zone → float above OSC

mp.register_script_message('mbv-skip-intro', function(end_secs_str)
    skip_intro.end_secs = tonumber(end_secs_str) or 0
    skip_intro.visible  = true
    skip_intro.render()
end)

mp.register_script_message('mbv-skip-intro-dismiss', function()
    skip_intro.hide()
end)

-- Dismiss on seek (user is navigating manually)
mp.register_event('seek', function()
    skip_intro.hide()
end)

-- Float above OSC when mouse enters the bottom zone where the OSC appears
mp.observe_property('mouse-pos', 'native', function(_, pos)
    if not skip_intro.visible then return end
    if not pos or not pos.hover then
        if skip_intro.mouse_near then
            skip_intro.mouse_near = false
            skip_intro.render()
        end
        return
    end
    local dim = mp.get_property_native('osd-dimensions')
    if not dim or dim.h <= 0 then return end
    local scale = osc_param.playresy / dim.h
    local vy    = pos.y * scale
    local near  = vy > osc_param.playresy * 0.78
    if near ~= skip_intro.mouse_near then
        skip_intro.mouse_near = near
        skip_intro.render()
    end
end)

mp.set_key_bindings({
    {'mbtn_left', function()
        local secs = skip_intro.end_secs
        skip_intro.hide()
        mp.set_property_number('time-pos', secs)
        mp.commandv('script-message', 'mbv-skip-intro-play')
    end},
}, 'skip-intro', 'force')
