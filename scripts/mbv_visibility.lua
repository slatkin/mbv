function always_on(val)
    if state.enabled then
        if val then
            show_osc()
        else
            hide_osc()
        end
    end
end

-- mode can be auto/always/never/cycle
-- the modes only affect internal variables and not stored on its own.
function visibility_mode(mode, no_osd)
    if mode == 'auto' then
        always_on(false)
        enable_osc(true)
    elseif mode == 'always' then
        enable_osc(true)
        always_on(true)
    elseif mode == 'never' then
        enable_osc(false)
    else
        msg.warn('Ignoring unknown visibility mode \'' .. mode .. '\'')
        return
    end

    user_opts.visibility = mode

    if not no_osd and tonumber(mp.get_property('osd-level')) >= 1 then
        mp.osd_message('OSC visibility: ' .. mode)
    end

    -- Reset the input state on a mode change. The input state will be
    -- recalcuated on the next render cycle, except in 'never' mode where it
    -- will just stay disabled.
    mp.disable_key_bindings('input')
    mp.disable_key_bindings('window-controls')
    state.input_enabled = false
    request_tick()
end

visibility_mode(user_opts.visibility, true)
mp.register_script_message('osc-visibility', visibility_mode)
mp.add_key_binding(nil, 'visibility', function() visibility_mode('cycle') end)

set_virt_mouse_area(0, 0, 0, 0, 'input')
set_virt_mouse_area(0, 0, 0, 0, 'window-controls')

--
-- Next-Up banner
-- Full-width bottom bar, same height as OSC, with DISMISS and SKIP buttons.
-- While visible the OSC is hidden and inaccessible.
--

local next_up = make_overlay(1001, {
    'next-up-skip',
    'next-up-dismiss',
}, function(pw, ph)
    if not next_up.visible then return nil end

    -- "NEXT UP" content state
    local show_t = next_up.show_title or ''
    local ep_t   = next_up.ep_title   or ''
    local art_t  = next_up.artist     or ''
    local label
    if show_t ~= '' then
        label = show_t .. ' - ' .. ep_t
    elseif art_t ~= '' then
        label = art_t .. ' - ' .. ep_t
    else
        label = ep_t
    end
    if #label > 56 then label = label:sub(1, 55) .. '...' end

    local pad = math.floor(pw * 0.03)  -- left/right padding

    -- Buttons: right-aligned, stacked vertically (SKIP on top, DISMISS below)
    local bar_h = 180
    local bar_y = ph - bar_h  -- top edge of overlay
    local btn_w   = math.max(110, math.floor(pw * 0.13))
    local btn_h   = math.max(30,  math.floor(ph / 24))
    local btn_r   = math.floor(btn_h / 3)
    local btn_gap = math.floor(bar_h * 0.08)
    local btn_x   = pw - pad - btn_w

    local total_btn_h = btn_h * 2 + btn_gap
    local btn_block_y = bar_y + math.floor((bar_h - total_btn_h) / 2)

    -- Font sizes: lbl+txt sum matches total button stack height so they align vertically
    local lbl_fs = math.max(10, math.floor(total_btn_h * 0.38))
    local txt_fs = total_btn_h - lbl_fs
    local btn_fs = math.max(12, math.floor(btn_h * 0.60))

    local skip_y1    = btn_block_y
    local skip_y2    = skip_y1 + btn_h
    local dismiss_y1 = skip_y2 + btn_gap
    local dismiss_y2 = dismiss_y1 + btn_h

    set_virt_mouse_area(btn_x,        skip_y1,
                        btn_x + btn_w, skip_y2,       'next-up-skip')
    set_virt_mouse_area(btn_x,        dismiss_y1,
                        btn_x + btn_w, dismiss_y2,    'next-up-dismiss')
    mp.enable_key_bindings('next-up-skip')
    mp.enable_key_bindings('next-up-dismiss')

    local ass = assdraw.ass_new()

    -- Semi-transparent black background (65 % opaque: alpha &H67& ≈ 40 % transparent)
    ass:new_event()
    ass:pos(0, bar_y)
    ass:an(7)
    ass:append('{\\bord0\\blur0\\1c&H000000&\\1a&H67&}')
    ass:draw_start()
    ass:rect_cw(0, 0, pw, bar_h)
    ass:draw_stop()

    -- Text block: left-aligned, vertically centred to match button stack
    local tx = pad
    local text_y = btn_block_y  -- top of text block aligns with top of button block

    -- "NEXT UP" label (top-left anchor)
    ass:new_event()
    ass:pos(tx, text_y)
    ass:an(7)
    ass:append(string.format('{\\fs%d\\bord0\\blur0\\1c&HA09090&\\bold0}', lbl_fs))
    ass:append('NEXT UP')

    -- Content line immediately below, no gap
    ass:new_event()
    ass:pos(tx, text_y + lbl_fs)
    ass:an(7)
    ass:append(string.format('{\\fs%d\\bord0\\blur0\\1c&HFAFAFA&\\bold1}', txt_fs))
    ass:append(label:gsub('{', '\\{'))

    -- SKIP button (green, top)
    ass:new_event()
    ass:pos(btn_x, skip_y1)
    ass:an(7)
    ass:append('{\\bord0\\blur0\\1c&H4BB552&\\1a&H00&}')
    ass:draw_start()
    ass:round_rect_cw(0, 0, btn_w, btn_h, btn_r, btn_r)
    ass:draw_stop()

    ass:new_event()
    ass:pos(btn_x + btn_w / 2, skip_y1 + btn_h / 2)
    ass:an(5)
    ass:append(string.format('{\\fs%d\\bord0\\blur0\\1c&HFAFAFA&\\bold1}', btn_fs))
    ass:append('SKIP')

    -- DISMISS button (dark grey, bottom)
    ass:new_event()
    ass:pos(btn_x, dismiss_y1)
    ass:an(7)
    ass:append('{\\bord0\\blur0\\1c&H555555&\\1a&H00&}')
    ass:draw_start()
    ass:round_rect_cw(0, 0, btn_w, btn_h, btn_r, btn_r)
    ass:draw_stop()

    ass:new_event()
    ass:pos(btn_x + btn_w / 2, dismiss_y1 + btn_h / 2)
    ass:an(5)
    ass:append(string.format('{\\fs%d\\bord0\\blur0\\1c&HFAFAFA&\\bold1}', btn_fs))
    ass:append('DISMISS')

    return ass.text
end)

next_up.item_id         = ''
next_up.show_title      = ''
next_up.ep_title        = ''
next_up.artist          = ''
next_up.osc_was_enabled = false

local function next_up_hide()
    next_up.hide(function()
        if next_up.osc_was_enabled then
            enable_osc(true)
        end
    end)
end

mp.register_script_message('mbv-next-up', function(item_id, show_title, ep_title, artist)
    msg.warn('next-up: received id=' .. tostring(item_id) .. ' show=' .. tostring(show_title) .. ' ep=' .. tostring(ep_title))
    next_up.item_id    = item_id    or ''
    next_up.show_title = show_title or ''
    next_up.ep_title   = ep_title   or ''
    next_up.artist     = artist     or ''
    if not next_up.visible then
        next_up.osc_was_enabled = state.enabled
        if state.enabled then enable_osc(false) end
    end
    next_up.visible = true
    next_up.render()
end)

mp.register_script_message('mbv-next-up-dismiss', function()
    next_up_hide()
end)

mp.set_key_bindings({
    {'mbtn_left', function() next_up_hide() end},
}, 'next-up-dismiss', 'force')

mp.set_key_bindings({
    {'mbtn_left', function()
        next_up_hide()
        mp.commandv('script-message', 'mbv-next-up-play')
    end},
}, 'next-up-skip', 'force')

-- Auto-dismiss when the next file actually starts playing.
mp.register_event('start-file', function()
    if next_up.visible then next_up_hide() end
end)

mp.observe_property('user-data/mbv/ep-tag', 'string', function(_, val)
    local v = val or ''
    ep_subtitle = v:gsub('^"(.*)"$', '%1')
    request_init()
end)
