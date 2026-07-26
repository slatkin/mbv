-- Element Management
--

local elements = {}

function prepare_elements()

    -- remove elements without layout or invisble
    local elements2 = {}
    for n, element in pairs(elements) do
        if not (element.layout == nil) and (element.visible) then
            table.insert(elements2, element)
        end
    end
    elements = elements2

    function elem_compare (a, b)
        return a.layout.layer < b.layout.layer
    end

    table.sort(elements, elem_compare)


    for _,element in pairs(elements) do

        local elem_geo = element.layout.geometry

        -- Calculate the hitbox
        local bX1, bY1, bX2, bY2 = get_hitbox_coords_geo(elem_geo)
        element.hitbox = {x1 = bX1, y1 = bY1, x2 = bX2, y2 = bY2}

        local style_ass = assdraw.ass_new()

        -- prepare static elements
        style_ass:append('{}') -- hack to troll new_event into inserting a \n
        style_ass:new_event()
        style_ass:pos(elem_geo.x, elem_geo.y)
        style_ass:an(elem_geo.an)
        style_ass:append(element.layout.style)

        element.style_ass = style_ass

        local static_ass = assdraw.ass_new()


        if (element.type == 'box') then
            --draw box
            static_ass:draw_start()
            ass_draw_rr_h_cw(static_ass, 0, 0, elem_geo.w, elem_geo.h,
                             element.layout.box.radius, element.layout.box.hexagon)
            static_ass:draw_stop()

        elseif (element.type == 'slider') then
            --draw static slider parts
            local slider_lo = element.layout.slider
            -- calculate positions of min and max points
            element.slider.min.ele_pos = elem_geo.h / 2
            element.slider.max.ele_pos = elem_geo.w - element.slider.min.ele_pos
            element.slider.min.glob_pos = element.hitbox.x1 + element.slider.min.ele_pos
            element.slider.max.glob_pos = element.hitbox.x1 + element.slider.max.ele_pos

            static_ass:draw_start()
            -- a hack which prepares the whole slider area to allow center placements such like an=5
            static_ass:rect_cw(0, 0, elem_geo.w, elem_geo.h)
            static_ass:rect_ccw(0, 0, elem_geo.w, elem_geo.h)
            -- marker nibbles
            if not (element.slider.markerF == nil) and (slider_lo.gap > 0) then
                local markers = element.slider.markerF()
                for _,marker in pairs(markers) do
                    if (marker >= element.slider.min.value) and (marker <= element.slider.max.value) then
                        local s = get_slider_ele_pos_for(element, marker)
                        if (slider_lo.gap > 5) then -- draw triangles
                            --top
                            if (slider_lo.nibbles_top) then
                                static_ass:move_to(s - 3, slider_lo.gap - 5)
                                static_ass:line_to(s + 3, slider_lo.gap - 5)
                                static_ass:line_to(s, slider_lo.gap - 1)
                            end
                            --bottom
                            if (slider_lo.nibbles_bottom) then
                                static_ass:move_to(s - 3, elem_geo.h - slider_lo.gap + 5)
                                static_ass:line_to(s, elem_geo.h - slider_lo.gap + 1)
                                static_ass:line_to(s + 3, elem_geo.h - slider_lo.gap + 5)
                            end
                        else -- draw 2x1px nibbles
                            --top
                            if (slider_lo.nibbles_top) then
                                static_ass:rect_cw(s - 1, 0, s + 1, slider_lo.gap);
                            end
                            --bottom
                            if (slider_lo.nibbles_bottom) then
                                static_ass:rect_cw(s - 1, elem_geo.h-slider_lo.gap, s + 1, elem_geo.h);
                            end
                        end
                    end
                end
            end
        end

        element.static_ass = static_ass

        -- if the element is supposed to be disabled,
        -- style it accordingly and kill the eventresponders
        if not (element.enabled) then
            element.layout.alpha[1] = 136
            element.eventresponder = nil
        end
    end
end

--
-- Element Rendering
--
function render_elements(master_ass)

    for n=1, #elements do
        local element = elements[n]
        local style_ass = assdraw.ass_new()
        style_ass:merge(element.style_ass)
        ass_append_alpha(style_ass, element.layout.alpha, 0)

        if element.eventresponder and (state.active_element == n) then
            -- run render event functions
            if not (element.eventresponder.render == nil) then
                element.eventresponder.render(element)
            end
            if mouse_hit(element) then
                -- mouse down styling
                if (element.styledown) then
                    style_ass:append(osc_styles.elementDown)
                end
                if (element.softrepeat) and (state.mouse_down_counter >= 15
                    and state.mouse_down_counter % 5 == 0) then

                    element.eventresponder[state.active_event_source..'_down'](element)
                end
                state.mouse_down_counter = state.mouse_down_counter + 1
            end
        end

        local elem_ass = assdraw.ass_new()
        elem_ass:merge(style_ass)

        if not (element.type == 'button') then
            elem_ass:merge(element.static_ass)
        end

        if (element.type == 'slider') then

            local slider_lo = element.layout.slider
            local elem_geo = element.layout.geometry
            local s_min = element.slider.min.value
            local s_max = element.slider.max.value
            -- draw pos marker
            local pos = element.slider.posF()
            local seekRanges
            local rh = elem_geo.h / 2 -- Handle radius
            local xp

            if pos then
                xp = get_slider_ele_pos_for(element, pos)
                ass_draw_cir_cw(elem_ass, xp, elem_geo.h/2, rh)
                elem_ass:rect_cw(0, slider_lo.gap, xp, elem_geo.h - slider_lo.gap)
            end
			if element.slider.seekRangesF ~= nil then seekRanges = element.slider.seekRangesF() end
            if seekRanges then
                elem_ass:draw_stop()
                elem_ass:merge(element.style_ass)
                ass_append_alpha(elem_ass, element.layout.alpha, user_opts.seekrangealpha)
                elem_ass:merge(element.static_ass)

                for _,range in pairs(seekRanges) do
                    local pstart = get_slider_ele_pos_for(element, range['start'])
                    local pend = get_slider_ele_pos_for(element, range['end'])
                    elem_ass:rect_cw(pstart - rh, slider_lo.gap, pend + rh, elem_geo.h - slider_lo.gap)
                end
            end

            elem_ass:draw_stop()

            -- add tooltip
            if not (element.slider.tooltipF == nil) then
                if mouse_hit(element) then
                    local sliderpos = get_slider_value(element)
                    local tooltiplabel = element.slider.tooltipF(sliderpos)
                    local an = slider_lo.tooltip_an
                    local ty
                    if (an == 2) then
                        ty = element.hitbox.y1
                    else
                        ty = element.hitbox.y1 + elem_geo.h/2
                    end

                    local tx = get_virt_mouse_pos()
                    if (slider_lo.adjust_tooltip) then
                        if (an == 2) then
                            if (sliderpos < (s_min + 3)) then
                                an = an - 1
                            elseif (sliderpos > (s_max - 3)) then
                                an = an + 1
                            end
                        elseif (sliderpos > (s_max-s_min)/2) then
                            an = an + 1
                            tx = tx - 5
                        else
                            an = an - 1
                            tx = tx + 10
                        end
                    end

                    -- tooltip label
                    elem_ass:new_event()
                    elem_ass:pos(tx, ty)
                    elem_ass:an(an)
                    elem_ass:append(slider_lo.tooltip_style)
                    ass_append_alpha(elem_ass, slider_lo.alpha, 0)
                    elem_ass:append(tooltiplabel)
                end
            end

        elseif (element.type == 'button') then

            local buttontext
            if type(element.content) == 'function' then
                buttontext = element.content() -- function objects
            elseif not (element.content == nil) then
                buttontext = element.content -- text objects
            end

            buttontext = buttontext:gsub(':%((.?.?.?)%) unknown ', ':%(%1%)')  --gsub('%) unknown %(\'', '')

            local maxchars = element.layout.button.maxchars
            -- 认为1个中文字符约等于1.5个英文字符
            local charcount = (buttontext:len() + select(2, buttontext:gsub('[^\128-\193]', ''))*2) / 3
            if not (maxchars == nil) and (charcount > maxchars) then
                local limit = math.max(0, maxchars - 3)
                if (charcount > limit) then
                    while (charcount > limit) do
                        buttontext = buttontext:gsub('.[\128-\191]*$', '')
                        charcount = (buttontext:len() + select(2, buttontext:gsub('[^\128-\193]', ''))*2) / 3
                    end
                    buttontext = buttontext .. '...'
                end
            end

            elem_ass:append(buttontext)

            -- add tooltip
            if not (element.tooltipF == nil) and element.enabled then
                if mouse_hit(element) then
                    local tooltiplabel = element.tooltipF
                    local an = 1
                    local ty = element.hitbox.y1
                    local tx = get_virt_mouse_pos()

                    if ty < osc_param.playresy / 2 then
                        ty = element.hitbox.y2
                        an = 7
                    end

                    -- tooltip label
                    if type(element.tooltipF) == 'function' then
                        tooltiplabel = element.tooltipF()
                    else
                        tooltiplabel = element.tooltipF
                    end
                    elem_ass:new_event()
                    elem_ass:pos(tx, ty)
                    elem_ass:an(an)
                    elem_ass:append(element.tooltip_style)
                    elem_ass:append(tooltiplabel)
                end
            end
        end

        master_ass:merge(elem_ass)
    end
end

--
-- Message display
--

-- pos is 1 based
function limited_list(prop, pos)
    local proplist = mp.get_property_native(prop, {})
    local count = #proplist
    if count == 0 then
        return count, proplist
    end

    local fs = tonumber(mp.get_property('options/osd-font-size'))
    local max = math.ceil(osc_param.unscaled_y*0.75 / fs)
    if max % 2 == 0 then
        max = max - 1
    end
    local delta = math.ceil(max / 2) - 1
    local begi = math.max(math.min(pos - delta, count - max + 1), 1)
    local endi = math.min(begi + max - 1, count)

    local reslist = {}
    for i=begi, endi do
        local item = proplist[i]
        item.current = (i == pos) and true or nil
        table.insert(reslist, item)
    end
    return count, reslist
end

function get_playlist()
    local pos = mp.get_property_number('playlist-pos', 0) + 1
    local count, limlist = limited_list('playlist', pos)
    if count == 0 then
        return texts.nolist
    end

    local message = string.format(texts.playlist .. ' [%d/%d]:\n', pos, count)
    for i, v in ipairs(limlist) do
        local title = v.title
        local _, filename = utils.split_path(v.filename)
        if title == nil then
            title = filename
        end
        message = string.format('%s %s %s\n', message,
            (v.current and '●' or '○'), title)
    end
    return message
end

function get_chapterlist()
    local pos = mp.get_property_number('chapter', 0) + 1
    local count, limlist = limited_list('chapter-list', pos)
    if count == 0 then
        return texts.nochapter
    end

    local message = string.format(texts.chapter.. ' [%d/%d]:\n', pos, count)
    for i, v in ipairs(limlist) do
        local time = mp.format_time(v.time)
        local title = v.title
        if title == nil then
            title = string.format(texts.chapter .. ' %02d', i)
        end
        message = string.format('%s[%s] %s %s\n', message, time,
            (v.current and '●' or '○'), title)
    end
    return message
end

function show_message(text, duration)

    --print('text: '..text..'   duration: ' .. duration)
    if duration == nil then
        duration = tonumber(mp.get_property('options/osd-duration')) / 1000
    elseif not type(duration) == 'number' then
        print('duration: ' .. duration)
    end

    -- cut the text short, otherwise the following functions
    -- may slow down massively on huge input
    text = string.sub(text, 0, 4000)

    -- replace actual linebreaks with ASS linebreaks
    text = string.gsub(text, '\n', '\\N')

    state.message_text = text

    if not state.message_hide_timer then
        state.message_hide_timer = mp.add_timeout(0, request_tick)
    end
    state.message_hide_timer:kill()
    state.message_hide_timer.timeout = duration
    state.message_hide_timer:resume()
    request_tick()
end

function render_message(ass)
    if state.message_hide_timer and state.message_hide_timer:is_enabled() and
       state.message_text
    then
        local _, lines = string.gsub(state.message_text, '\\N', '')

        local fontsize = tonumber(mp.get_property('options/osd-font-size'))
        local outline = tonumber(mp.get_property('options/osd-border-size'))
        local maxlines = math.ceil(osc_param.unscaled_y*0.75 / fontsize)
        local counterscale = osc_param.playresy / osc_param.unscaled_y

        fontsize = fontsize * counterscale / math.max(0.65 + math.min(lines/maxlines, 1), 1)
        outline = outline * counterscale / math.max(0.75 + math.min(lines/maxlines, 1)/2, 1)

        local style = '{\\bord' .. outline .. '\\fs' .. fontsize .. '}'


        ass:new_event()
        ass:append(style .. state.message_text)
    else
        state.message_text = nil
    end
end

--
-- Initialisation and Layout
--

function new_element(name, type)
    elements[name] = {}
    elements[name].type = type

    -- add default stuff
    elements[name].eventresponder = {}
    elements[name].visible = true
    elements[name].enabled = true
    elements[name].softrepeat = false
    elements[name].styledown = (type == 'button')
    elements[name].state = {}

    if (type == 'slider') then
        elements[name].slider = {min = {value = 0}, max = {value = 100}}
    end


    return elements[name]
end

function add_layout(name)
    if not (elements[name] == nil) then
        -- new layout
        elements[name].layout = {}

        -- set layout defaults
        elements[name].layout.layer = 50
        elements[name].layout.alpha = {[1] = 0, [2] = 255, [3] = 255, [4] = 255}

        if (elements[name].type == 'button') then
            elements[name].layout.button = {
                maxchars = nil,
            }
        elseif (elements[name].type == 'slider') then
            -- slider defaults
            elements[name].layout.slider = {
                border = 1,
                gap = 1,
                nibbles_top = true,
                nibbles_bottom = true,
                adjust_tooltip = true,
                tooltip_style = '',
                tooltip_an = 2,
                alpha = {[1] = 0, [2] = 255, [3] = 88, [4] = 255},
            }
        elseif (elements[name].type == 'box') then
            elements[name].layout.box = {radius = 0, hexagon = false}
        end

        return elements[name].layout
    else
        msg.error('Can\'t add_layout to element \''..name..'\', doesn\'t exist.')
    end
end

-- Window Controls
function window_controls()
    local wc_geo = {
        x = 0,
        y = 32,
        an = 1,
        w = osc_param.playresx,
        h = 32,
    }

    local controlbox_w = window_control_box_width
    local titlebox_w = wc_geo.w - controlbox_w

    -- Default alignment is 'right'
    local controlbox_left = wc_geo.w - controlbox_w
    local titlebox_left = wc_geo.x
    local titlebox_right = wc_geo.w - controlbox_w

    add_area('window-controls',
             get_hitbox_coords(controlbox_left, wc_geo.y, wc_geo.an,
                               controlbox_w, wc_geo.h))

    local lo

    local button_y = wc_geo.y - (wc_geo.h / 2)
    local first_geo =
        {x = controlbox_left + 27, y = button_y, an = 5, w = 40, h = wc_geo.h}
    local second_geo =
        {x = controlbox_left + 69, y = button_y, an = 5, w = 40, h = wc_geo.h}
    local third_geo =
        {x = controlbox_left + 115, y = button_y, an = 5, w = 40, h = wc_geo.h}

    -- Window control buttons use symbols in the custom mpv osd font
    -- because the official unicode codepoints are sufficiently
    -- exotic that a system might lack an installed font with them,
    -- and libass will complain that they are not present in the
    -- default font, even if another font with them is available.

    -- Close: ??
    ne = new_element('close', 'button')
    ne.content = '\238\132\149'
    ne.eventresponder['mbtn_left_up'] =
        function () mp.commandv('quit') end
    lo = add_layout('close')
    lo.geometry = third_geo
    lo.style = osc_styles.WinCtrl
    lo.alpha[3] = 0

    -- Minimize: ??
    ne = new_element('minimize', 'button')
    ne.content = '\\n\238\132\146'
    ne.eventresponder['mbtn_left_up'] =
        function () mp.commandv('cycle', 'window-minimized') end
    lo = add_layout('minimize')
    lo.geometry = first_geo
    lo.style = osc_styles.WinCtrl
    lo.alpha[3] = 0

    -- Maximize: ?? /??
    ne = new_element('maximize', 'button')
    if state.maximized or state.fullscreen then
        ne.content = '\238\132\148'
    else
        ne.content = '\238\132\147'
    end
    ne.eventresponder['mbtn_left_up'] =
        function ()
            if state.fullscreen then
                mp.commandv('cycle', 'fullscreen')
            else
                mp.commandv('cycle', 'window-maximized')
            end
        end
    lo = add_layout('maximize')
    lo.geometry = second_geo
    lo.style = osc_styles.WinCtrl
    lo.alpha[3] = 0
end

--
-- Layouts
--

local layouts = {}

-- Default layout
layouts = function ()

    local osc_geo = {w, h}

    osc_geo.w = osc_param.playresx
    osc_geo.h = 180

    -- origin of the controllers, left/bottom corner
    local posX = 0
    local posY = osc_param.playresy

    osc_param.areas = {} -- delete areas

    -- area for active mouse input
    add_area('input', get_hitbox_coords(posX, posY, 1, osc_geo.w, 104))

    -- area for show/hide
    add_area('showhide', 0, 0, osc_param.playresx, osc_param.playresy)
    add_area('showhide_wc', osc_param.playresx*0.67, 0, osc_param.playresx, 48)

    -- fetch values
    local osc_w, osc_h=
        osc_geo.w, osc_geo.h

    --
    -- Controller Background
    --
    local lo

    new_element('TransBg', 'box')
    lo = add_layout('TransBg')
    lo.geometry = {x = posX, y = posY, an = 7, w = osc_w, h = 1}
    lo.style = osc_styles.TransBg
    lo.layer = 10
    lo.alpha[3] = 0

    --
    -- Alignment
    --
    local refX = osc_w / 2
    local refY = posY
    local geo

    --
    -- Seekbar
    --
    new_element('seekbarbg', 'box')
    lo = add_layout('seekbarbg')
    lo.geometry = {x = refX , y = refY - 96 , an = 5, w = osc_geo.w - 50, h = 2}
    lo.layer = 13
    lo.style = osc_styles.SeekbarBg
    lo.alpha[1] = 128
    lo.alpha[3] = 128

    lo = add_layout('seekbar')
    lo.geometry = {x = refX, y = refY - 96 , an = 5, w = osc_geo.w - 50, h = 16}
    lo.style = osc_styles.SeekbarFg
    lo.slider.gap = 7
    lo.slider.tooltip_style = osc_styles.Tooltip
    lo.slider.tooltip_an = 2
    --
    -- Volumebar
    --
    lo = new_element('volumebarbg', 'box')
    lo.visible = (osc_param.playresx >= 750) and user_opts.volumecontrol
    lo = add_layout('volumebarbg')
    lo.geometry = {x = 155, y = refY - 40, an = 4, w = 80, h = 2}
    lo.layer = 13
    lo.style = osc_styles.VolumebarBg


    lo = add_layout('volumebar')
    lo.geometry = {x = 155, y = refY - 40, an = 4, w = 80, h = 8}
    lo.style = osc_styles.VolumebarFg
    lo.slider.gap = 3
    lo.slider.tooltip_style = osc_styles.Tooltip
    lo.slider.tooltip_an = 2

    -- buttons
    lo = add_layout('pl_prev')
    lo.geometry = {x = refX - 120, y = refY - 40 , an = 5, w = 30, h = 24}
    lo.style = osc_styles.Ctrl2

    lo = add_layout('skipback')
    lo.geometry = {x = refX - 60, y = refY - 40 , an = 5, w = 30, h = 24}
    lo.style = osc_styles.Ctrl2


    lo = add_layout('playpause')
    lo.geometry = {x = refX, y = refY - 40 , an = 5, w = 45, h = 45}
    lo.style = osc_styles.Ctrl1

    lo = add_layout('skipfrwd')
    lo.geometry = {x = refX + 60, y = refY - 40 , an = 5, w = 30, h = 24}
    lo.style = osc_styles.Ctrl2

    lo = add_layout('pl_next')
    lo.geometry = {x = refX + 120, y = refY - 40 , an = 5, w = 30, h = 24}
    lo.style = osc_styles.Ctrl2


    -- Time
    lo = add_layout('tc_left')
    lo.geometry = {x = 25, y = refY - 84, an = 7, w = 64, h = 20}
    lo.style = osc_styles.Time


    lo = add_layout('tc_right')
    lo.geometry = {x = osc_geo.w - 25 , y = refY -84, an = 9, w = 64, h = 20}
    lo.style = osc_styles.Time

    lo = add_layout('cy_audio')
    lo.geometry = {x = 37, y = refY - 40, an = 5, w = 24, h = 24}
    lo.style = osc_styles.Ctrl3
    lo.visible = (osc_param.playresx >= 540)

    lo = add_layout('cy_sub')
    lo.geometry = {x = 87, y = refY - 40, an = 5, w = 24, h = 24}
    lo.style = osc_styles.Ctrl3
    lo.visible = (osc_param.playresx >= 600)

    lo = add_layout('vol_ctrl')
    lo.geometry = {x = 137, y = refY - 40, an = 5, w = 24, h = 24}
    lo.style = osc_styles.Ctrl3
    lo.visible = (osc_param.playresx >= 650)

    lo = add_layout('tog_fs')
    lo.geometry = {x = osc_geo.w - 37, y = refY - 40, an = 5, w = 24, h = 24}
    lo.style = osc_styles.Ctrl3
    lo.visible = (osc_param.playresx >= 540)

    lo = add_layout('tog_info')
    lo.geometry = {x = osc_geo.w - 87, y = refY - 40, an = 5, w = 24, h = 24}
    lo.style = osc_styles.Ctrl3
    lo.visible = (osc_param.playresx >= 600)

    geo = { x = 25, y = refY - 132, an = 1, w = osc_geo.w - 50, h = 48 }
    lo = add_layout('title')
    lo.geometry = geo
    lo.style = string.format('%s{\\clip(%f,%f,%f,%f)}', osc_styles.Title,
                                geo.x, geo.y - geo.h, geo.x + geo.w , geo.y)
    lo.alpha[3] = 0
    lo.button.maxchars = geo.w / 23

    lo = add_layout('ep_subtitle')
    lo.geometry = { x = 25, y = refY - 140, an = 7, w = osc_geo.w - 50, h = 22 }
    lo.style = '{\\blur0\\bord0.5\\1c&HFFFFFF&\\3c&H000000&\\fs30\\fn' .. user_opts.font .. '}'
    lo.alpha[3] = 0
end
