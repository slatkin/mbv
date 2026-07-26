function shutdown()

end

--
-- Other important stuff
--


function show_osc()
    -- show when disabled can happen (e.g. mouse_move) due to async/delayed unbinding
    if not state.enabled then return end

    msg.trace('show_osc')
    --remember last time of invocation (mouse move)
    state.showtime = mp.get_time()

    osc_visible(true)

    if (user_opts.fadeduration > 0) then
        state.anitype = nil
    end
end

function hide_osc()
    msg.trace('hide_osc')
    if not state.enabled then
        -- typically hide happens at render() from tick(), but now tick() is
        -- no-op and won't render again to remove the osc, so do that manually.
        state.osc_visible = false
        render_wipe()
    elseif (user_opts.fadeduration > 0) then
        if not(state.osc_visible == false) then
            state.anitype = 'out'
            request_tick()
        end
    else
        osc_visible(false)
    end
end

function osc_visible(visible)
    if state.osc_visible ~= visible then
        state.osc_visible = visible
    end
    request_tick()
end

function pause_state(name, enabled)
    state.paused = enabled
    if user_opts.showtitle then
        if enabled then
            state.lastvisibility = user_opts.visibility
            visibility_mode('always', true)
            show_osc()
        else
            visibility_mode(state.lastvisibility, true)
        end
    end
    request_tick()
end

function cache_state(name, st)
    state.cache_state = st
    request_tick()
end

-- Request that tick() is called (which typically re-renders the OSC).
-- The tick is then either executed immediately, or rate-limited if it was
-- called a small time ago.
function request_tick()
    if state.tick_timer == nil then
        state.tick_timer = mp.add_timeout(0, tick)
    end

    if not state.tick_timer:is_enabled() then
        local now = mp.get_time()
        local timeout = tick_delay - (now - state.tick_last_time)
        if timeout < 0 then
            timeout = 0
        end
        state.tick_timer.timeout = timeout
        state.tick_timer:resume()
    end
end

function mouse_leave()
    if get_hidetimeout() >= 0 then
        hide_osc()
    end
    -- reset mouse position
    state.last_mouseX, state.last_mouseY = nil, nil
    state.mouse_in_window = false
end

function request_init()
    state.initREQ = true
    request_tick()
end

-- Like request_init(), but also request an immediate update
function request_init_resize()
    request_init()
    -- ensure immediate update
    state.tick_timer:kill()
    state.tick_timer.timeout = 0
    state.tick_timer:resume()
end

function render_wipe()
    msg.trace('render_wipe()')
    state.osd:remove()
end

function render()
    msg.trace('rendering')
    local current_screen_sizeX, current_screen_sizeY, aspect = mp.get_osd_size()
    local mouseX, mouseY = get_virt_mouse_pos()
    local now = mp.get_time()

    -- check if display changed, if so request reinit
    if not (state.mp_screen_sizeX == current_screen_sizeX
        and state.mp_screen_sizeY == current_screen_sizeY) then

        request_init_resize()

        state.mp_screen_sizeX = current_screen_sizeX
        state.mp_screen_sizeY = current_screen_sizeY
    end

    -- init management
    if state.initREQ then
        osc_init()
        state.initREQ = false

        -- store initial mouse position
        if (state.last_mouseX == nil or state.last_mouseY == nil)
            and not (mouseX == nil or mouseY == nil) then

            state.last_mouseX, state.last_mouseY = mouseX, mouseY
        end
    end


    -- fade animation
    if not(state.anitype == nil) then

        if (state.anistart == nil) then
            state.anistart = now
        end

        if (now < state.anistart + (user_opts.fadeduration/1000)) then

            if (state.anitype == 'in') then --fade in
                osc_visible(true)
                state.animation = scale_value(state.anistart,
                    (state.anistart + (user_opts.fadeduration/1000)),
                    255, 0, now)
            elseif (state.anitype == 'out') then --fade out
                state.animation = scale_value(state.anistart,
                    (state.anistart + (user_opts.fadeduration/1000)),
                    0, 255, now)
            end

        else
            if (state.anitype == 'out') then
                osc_visible(false)
            end
            state.anistart = nil
            state.animation = nil
            state.anitype =  nil
        end
    else
        state.anistart = nil
        state.animation = nil
        state.anitype =  nil
    end

    --mouse show/hide area
    for k,cords in pairs(osc_param.areas['showhide']) do
        set_virt_mouse_area(cords.x1, cords.y1, cords.x2, cords.y2, 'showhide')
    end
    if osc_param.areas['showhide_wc'] then
        for k,cords in pairs(osc_param.areas['showhide_wc']) do
            set_virt_mouse_area(cords.x1, cords.y1, cords.x2, cords.y2, 'showhide_wc')
        end
    else
        set_virt_mouse_area(0, 0, 0, 0, 'showhide_wc')
    end
    do_enable_keybindings()

    --mouse input area
    local mouse_over_osc = false

    for _,cords in ipairs(osc_param.areas['input']) do
        if state.osc_visible then -- activate only when OSC is actually visible
            set_virt_mouse_area(cords.x1, cords.y1, cords.x2, cords.y2, 'input')
        end
        if state.osc_visible ~= state.input_enabled then
            if state.osc_visible then
                mp.enable_key_bindings('input')
            else
                mp.disable_key_bindings('input')
            end
            state.input_enabled = state.osc_visible
        end

        if (mouse_hit_coords(cords.x1, cords.y1, cords.x2, cords.y2)) then
            mouse_over_osc = true
        end
    end

    if osc_param.areas['window-controls'] then
        for _,cords in ipairs(osc_param.areas['window-controls']) do
            if state.osc_visible then -- activate only when OSC is actually visible
                set_virt_mouse_area(cords.x1, cords.y1, cords.x2, cords.y2, 'window-controls')
                mp.enable_key_bindings('window-controls')
            else
                mp.disable_key_bindings('window-controls')
            end

            if (mouse_hit_coords(cords.x1, cords.y1, cords.x2, cords.y2)) then
                mouse_over_osc = true
            end
        end
    end

    if osc_param.areas['window-controls-title'] then
        for _,cords in ipairs(osc_param.areas['window-controls-title']) do
            if (mouse_hit_coords(cords.x1, cords.y1, cords.x2, cords.y2)) then
                mouse_over_osc = true
            end
        end
    end

    -- autohide
    if not (state.showtime == nil) and (get_hidetimeout() >= 0) then
        local timeout = state.showtime + (get_hidetimeout()/1000) - now
        if timeout <= 0 then
            if (state.active_element == nil) and not (mouse_over_osc) then
                hide_osc()
            end
        else
            -- the timer is only used to recheck the state and to possibly run
            -- the code above again
            if not state.hide_timer then
                state.hide_timer = mp.add_timeout(0, tick)
            end
            state.hide_timer.timeout = timeout
            -- re-arm
            state.hide_timer:kill()
            state.hide_timer:resume()
        end
    end


    -- actual rendering
    local ass = assdraw.ass_new()

    -- Messages
    render_message(ass)

    -- actual OSC
    if state.osc_visible then
        render_elements(ass)
    end

    -- submit
    set_osd(osc_param.playresy * osc_param.display_aspect,
            osc_param.playresy, ass.text)
end

--
-- Eventhandling
--

local function element_has_action(element, action)
    return element and element.eventresponder and
        element.eventresponder[action]
end
