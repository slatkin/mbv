-- Skip Intro overlay
local skip_intro = {
    visible    = false,
    end_secs   = 0,
    mouse_near = false,  -- true when mouse is in the OSC zone → float above OSC
    osd        = mp.create_osd_overlay('ass-events'),
    x1 = 0, y1 = 0, x2 = 0, y2 = 0,
}

local function skip_intro_hide()
    if not skip_intro.visible then return end
    skip_intro.visible = false
    skip_intro.osd.data = ''
    skip_intro.osd:update()
    set_virt_mouse_area(0, 0, 0, 0, 'skip-intro')
    mp.disable_key_bindings('skip-intro')
end

local function skip_intro_render()
    if not skip_intro.visible then return end

    local pw = osc_param.playresx
    local ph = osc_param.playresy

    -- osc_param is 0 until the OSC render loop first fires; at intro-start=0 the
    -- script-message arrives before that happens, so fall back to raw pixel dims.
    if pw <= 0 or ph <= 0 then
        local dim = mp.get_property_native('osd-dimensions')
        if not dim or dim.w <= 0 or dim.h <= 0 then return end
        pw = dim.w
        ph = dim.h
    end

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

    skip_intro.x1 = bx
    skip_intro.y1 = by
    skip_intro.x2 = bx + bw
    skip_intro.y2 = by + bh

    set_virt_mouse_area(skip_intro.x1, skip_intro.y1, skip_intro.x2, skip_intro.y2, 'skip-intro')
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

    skip_intro.osd.res_x = pw
    skip_intro.osd.res_y = ph
    skip_intro.osd.data  = ass.text
    skip_intro.osd.z     = 999
    skip_intro.osd:update()
end

mp.register_script_message('mbv-skip-intro', function(end_secs_str)
    skip_intro.end_secs = tonumber(end_secs_str) or 0
    skip_intro.visible  = true
    skip_intro_render()
end)

mp.register_script_message('mbv-skip-intro-dismiss', function()
    skip_intro_hide()
end)

-- Dismiss on seek (user is navigating manually)
mp.register_event('seek', function()
    skip_intro_hide()
end)

-- Re-render on window resize
mp.observe_property('osd-dimensions', 'native', function()
    if skip_intro.visible then skip_intro_render() end
end)

-- Float above OSC when mouse enters the bottom zone where the OSC appears
mp.observe_property('mouse-pos', 'native', function(_, pos)
    if not skip_intro.visible then return end
    if not pos or not pos.hover then
        if skip_intro.mouse_near then
            skip_intro.mouse_near = false
            skip_intro_render()
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
        skip_intro_render()
    end
end)

mp.set_key_bindings({
    {'mbtn_left', function()
        local secs = skip_intro.end_secs
        skip_intro_hide()
        mp.set_property_number('time-pos', secs)
        mp.commandv('script-message', 'mbv-skip-intro-play')
    end},
}, 'skip-intro', 'force')
