function process_event(source, what)
    local action = string.format('%s%s', source,
        what and ('_' .. what) or '')

    if what == 'down' or what == 'press' then

        for n = 1, #elements do

            if mouse_hit(elements[n]) and
                elements[n].eventresponder and
                (elements[n].eventresponder[source .. '_up'] or
                    elements[n].eventresponder[action]) then

                if what == 'down' then
                    state.active_element = n
                    state.active_event_source = source
                end
                -- fire the down or press event if the element has one
                if element_has_action(elements[n], action) then
                    elements[n].eventresponder[action](elements[n])
                end

            end
        end

    elseif what == 'up' then

        if elements[state.active_element] then
            local n = state.active_element

            if n == 0 then
                --click on background (does not work)
            elseif element_has_action(elements[n], action) and
                mouse_hit(elements[n]) then

                elements[n].eventresponder[action](elements[n])
            end

            --reset active element
            if element_has_action(elements[n], 'reset') then
                elements[n].eventresponder['reset'](elements[n])
            end

        end
        state.active_element = nil
        state.mouse_down_counter = 0

    elseif source == 'mouse_move' then

        state.mouse_in_window = true

        local mouseX, mouseY = get_virt_mouse_pos()
        if (user_opts.minmousemove == 0) or
            (not ((state.last_mouseX == nil) or (state.last_mouseY == nil)) and
                ((math.abs(mouseX - state.last_mouseX) >= user_opts.minmousemove)
                    or (math.abs(mouseY - state.last_mouseY) >= user_opts.minmousemove)
                )
            ) then
            show_osc()
        end
        state.last_mouseX, state.last_mouseY = mouseX, mouseY

        local n = state.active_element
        if element_has_action(elements[n], action) then
            elements[n].eventresponder[action](elements[n])
        end
        request_tick()
    end
end

function show_logo()
    local osd_w, osd_h, osd_aspect = mp.get_osd_size()
    osd_w, osd_h = 360*osd_aspect, 360
    local logo_x, logo_y = osd_w/2, osd_h/2-20
    local ass = assdraw.ass_new()
    ass:new_event()
    ass:pos(logo_x, logo_y)
    ass:append('{\\1c&HDCA400&\\3c&H0&\\3a&H60&\\blur1\\bord0.5}')
    ass:draw_start()
    ass_draw_cir_cw(ass, 0, 0, 100)
    ass:draw_stop()

    ass:new_event()
    ass:pos(logo_x, logo_y)
    ass:append('{\\1c&H4BB552&\\bord0}')
    ass:draw_start()
    ass_draw_cir_cw(ass, 6, -6, 75)
    ass:draw_stop()

    ass:new_event()
    ass:pos(logo_x, logo_y)
    ass:append('{\\1c&HFFFFFF&\\bord0}')
    ass:draw_start()
    ass_draw_cir_cw(ass, -4, 4, 50)
    ass:draw_stop()

    ass:new_event()
    ass:pos(logo_x, logo_y)
    ass:append('{\\1c&H4BB552&\\bord&}')
    ass:draw_start()
    ass:move_to(-20, -20)
    ass:line_to(23.3, 5)
    ass:line_to(-20, 30)
    ass:draw_stop()

    ass:new_event()
    ass:pos(logo_x, logo_y+110)
    ass:an(8)
    ass:append(texts.welcome)
    set_osd(osd_w, osd_h, ass.text)
end

-- called by mpv on every frame
function tick()
    if (not state.enabled) then return end

    if (state.idle) then
        show_logo()
        -- render idle message
        msg.trace('idle message')

        if state.showhide_enabled then
            mp.disable_key_bindings('showhide')
            mp.disable_key_bindings('showhide_wc')
            state.showhide_enabled = false
        end


    elseif (state.fullscreen and user_opts.showfullscreen)
        or (not state.fullscreen and user_opts.showwindowed) then

        -- render the OSC
        render()
    else
        -- Flush OSD
        set_osd(osc_param.playresy, osc_param.playresy, '')
    end

    state.tick_last_time = mp.get_time()

    if state.anitype ~= nil then
        request_tick()
    end
end

function do_enable_keybindings()
    if state.enabled then
        if not state.showhide_enabled then
            mp.enable_key_bindings('showhide', 'allow-vo-dragging+allow-hide-cursor')
            mp.enable_key_bindings('showhide_wc', 'allow-vo-dragging+allow-hide-cursor')
        end
        state.showhide_enabled = true
    end
end

function enable_osc(enable)
    state.enabled = enable
    if enable then
        do_enable_keybindings()
    else
        hide_osc() -- acts immediately when state.enabled == false
        if state.showhide_enabled then
            mp.disable_key_bindings('showhide')
            mp.disable_key_bindings('showhide_wc')
        end
        state.showhide_enabled = false
    end
end

validate_user_opts()

mp.register_event('shutdown', shutdown)
mp.register_event('start-file', request_init)
mp.register_event('seek', show_osc)
mp.observe_property('track-list', nil, request_init)
mp.observe_property('playlist', nil, request_init)

mp.register_script_message('osc-message', show_message)
mp.register_script_message('osc-chapterlist', function(dur)
    show_message(get_chapterlist(), dur)
end)
mp.register_script_message('osc-playlist', function(dur)
    show_message(get_playlist(), dur)
end)
mp.register_script_message('osc-tracklist', function(dur)
    local msg = {}
    for k,v in pairs(nicetypes) do
        table.insert(msg, get_tracklist(k))
    end
    show_message(table.concat(msg, '\n\n'), dur)
end)

mp.observe_property('fullscreen', 'bool',
    function(name, val)
        state.fullscreen = val
        request_init_resize()
    end
)
mp.observe_property('mute', 'bool',
    function(name, val)
        state.mute = val
    end
)
mp.observe_property('volume', 'number',
	function(name, val)
		state.sys_volume = val
		if user_opts.processvolume then
			state.proc_volume = val*val/100
		else
			state.proc_volume = val
		end
	end
)
mp.observe_property('border', 'bool',
    function(name, val)
        state.border = val
        request_init_resize()
    end
)
mp.observe_property('window-maximized', 'bool',
    function(name, val)
        state.maximized = val
        request_init_resize()
    end
)
mp.observe_property('idle-active', 'bool',
    function(name, val)
        state.idle = val
        request_tick()
    end
)
mp.observe_property('pause', 'bool', pause_state)
mp.observe_property('demuxer-cache-state', 'native', cache_state)
mp.observe_property('vo-configured', 'bool', function(name, val)
    request_tick()
end)
mp.observe_property('playback-time', 'number', function(name, val)
    request_tick()
end)
mp.observe_property('osd-dimensions', 'native', function(name, val)
    -- (we could use the value instead of re-querying it all the time, but then
    --  we might have to worry about property update ordering)
    request_init_resize()
end)

-- mouse show/hide bindings
mp.set_key_bindings({
    {'mouse_move',              function(e) process_event('mouse_move', nil) end},
    {'mouse_leave',             mouse_leave},
}, 'showhide', 'force')
mp.set_key_bindings({
    {'mouse_move',              function(e) process_event('mouse_move', nil) end},
    {'mouse_leave',             mouse_leave},
}, 'showhide_wc', 'force')
do_enable_keybindings()

--mouse input bindings
mp.set_key_bindings({
    {'mbtn_left',           function(e) process_event('mbtn_left', 'up') end,
                            function(e) process_event('mbtn_left', 'down')  end},
    {'mbtn_right',          function(e) process_event('mbtn_right', 'up') end,
                            function(e) process_event('mbtn_right', 'down')  end},
    {'mbtn_mid',            function(e) process_event('mbtn_mid', 'up') end,
                            function(e) process_event('mbtn_mid', 'down')  end},
    {'wheel_up',            function(e) process_event('wheel_up', 'press') end},
    {'wheel_down',          function(e) process_event('wheel_down', 'press') end},
    {'mbtn_left_dbl',       'ignore'},
    {'mbtn_right_dbl',      'ignore'},
}, 'input', 'force')
mp.enable_key_bindings('input')

mp.set_key_bindings({
    {'mbtn_left',           function(e) process_event('mbtn_left', 'up') end,
                            function(e) process_event('mbtn_left', 'down')  end},
}, 'window-controls', 'force')
mp.enable_key_bindings('window-controls')

function get_hidetimeout()
    if user_opts.visibility == 'always' then
        return -1 -- disable autohide
    end
    return user_opts.hidetimeout
end
