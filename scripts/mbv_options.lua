-- Validate string type user options
function validate_user_opts()
    if user_opts.windowcontrols ~= 'auto' and
       user_opts.windowcontrols ~= 'yes' and
       user_opts.windowcontrols ~= 'no' then
        msg.warn('windowcontrols cannot be \'' ..
                user_opts.windowcontrols .. '\'. Ignoring.')
        user_opts.windowcontrols = 'auto'
    end
end

function update_options(list)
    validate_user_opts()
    request_tick()
    visibility_mode(user_opts.visibility, true)
    request_init()
end

-- OSC INIT
function osc_init()
    msg.debug('osc_init')

    -- set canvas resolution according to display aspect and scaling setting
    local baseResY = 720
    local display_w, display_h, display_aspect = mp.get_osd_size()
    local scale = 1

    if (mp.get_property('video') == 'no') then -- dummy/forced window
        scale = user_opts.scaleforcedwindow
    elseif state.fullscreen then
        scale = user_opts.scalefullscreen
    else
        scale = user_opts.scalewindowed
    end

    if user_opts.vidscale then
        osc_param.unscaled_y = baseResY
    else
        osc_param.unscaled_y = display_h
    end
    osc_param.playresy = osc_param.unscaled_y / scale
    if (display_aspect > 0) then
        osc_param.display_aspect = display_aspect
    end
    osc_param.playresx = osc_param.playresy * osc_param.display_aspect

    -- stop seeking with the slider to prevent skipping files
    state.active_element = nil

    elements = {}

    -- some often needed stuff
    local pl_count = mp.get_property_number('playlist-count', 0)
    local have_pl = (pl_count > 1)
    local pl_pos = mp.get_property_number('playlist-pos', 0) + 1
    local have_ch = (mp.get_property_number('chapters', 0) > 0)
    local loop = mp.get_property('loop-playlist', 'no')

    local ne

    -- playlist buttons
    -- prev
    ne = new_element('pl_prev', 'button')

    ne.content = '\xEF\x8E\xB5'
    ne.enabled = (pl_pos > 1) or (loop ~= 'no')
    ne.eventresponder['mbtn_left_up'] =
        function ()
            mp.commandv('playlist-prev', 'weak')
        end
    ne.eventresponder['mbtn_right_up'] =
        function () show_message(get_playlist()) end

    --next
    ne = new_element('pl_next', 'button')

    ne.content = '\xEF\x8E\xB4'
    ne.enabled = (have_pl and (pl_pos < pl_count)) or (loop ~= 'no')
    ne.eventresponder['mbtn_left_up'] =
        function ()
            mp.commandv('playlist-next', 'weak')
        end
    ne.eventresponder['mbtn_right_up'] =
        function () show_message(get_playlist()) end


    --play control buttons
    --playpause
    ne = new_element('playpause', 'button')

    ne.content = function ()
        if mp.get_property('pause') == 'no' then
            return ('\xEF\x8E\xA7')
        else
            return ('\xEF\x8E\xAA')
        end
    end
    ne.eventresponder['mbtn_left_up'] =
        function () mp.commandv('cycle', 'pause') end
    --ne.eventresponder['mbtn_right_up'] =
    --    function () mp.commandv('script-binding', 'open-file-dialog') end

    --skipback
    ne = new_element('skipback', 'button')

    ne.softrepeat = true
    ne.content = '\xEF\x8E\xA0'
    ne.eventresponder['mbtn_left_down'] =
        --function () mp.command('seek -5') end
        function () mp.commandv('seek', -5, 'relative', 'keyframes') end
    ne.eventresponder['shift+mbtn_left_down'] =
        function () mp.commandv('frame-back-step') end
    ne.eventresponder['mbtn_right_down'] =
        --function () mp.command('seek -60') end
        function () mp.commandv('seek', -60, 'relative', 'keyframes') end

    --skipfrwd
    ne = new_element('skipfrwd', 'button')

    ne.softrepeat = true
    ne.content = '\xEF\x8E\x9F'
    ne.eventresponder['mbtn_left_down'] =
        --function () mp.command('seek +5') end
        function () mp.commandv('seek', 5, 'relative', 'keyframes') end
    ne.eventresponder['shift+mbtn_left_down'] =
        function () mp.commandv('frame-step') end
    ne.eventresponder['mbtn_right_down'] =
        --function () mp.command('seek +60') end
        function () mp.commandv('seek', 60, 'relative', 'keyframes') end

    --
    update_tracklist()

    --cy_audio
    ne = new_element('cy_audio', 'button')
    ne.enabled = (#tracks_osc.audio > 0)
    ne.visible = (osc_param.playresx >= 540)
    ne.content = '\xEF\x8E\xB7'
    ne.tooltip_style = osc_styles.Tooltip
    ne.tooltipF = function ()
        local msg = texts.off
        if not (get_track('audio') == 0) then
            msg = (texts.audio .. ' [' .. get_track('audio') .. ' ∕ ' .. #tracks_osc.audio .. '] ')
            local prop = mp.get_property('current-tracks/audio/lang')
            if not prop then
                prop = texts.na
            end
            msg = msg .. '[' .. prop .. ']'
            prop = mp.get_property('current-tracks/audio/title')
            if prop then
                msg = msg .. ' ' .. prop
            end
            return msg
        end
        return msg
    end
    ne.eventresponder['mbtn_left_up'] =
        function () set_track('audio', 1) end
    ne.eventresponder['mbtn_right_up'] =
        function () set_track('audio', -1) end
    ne.eventresponder['mbtn_mid_up'] =
        function () show_message(get_tracklist('audio')) end

    --cy_sub
    ne = new_element('cy_sub', 'button')
    ne.enabled = (#tracks_osc.sub > 0)
    ne.visible = (osc_param.playresx >= 600)
    ne.content = '\xEF\x8F\x93'
    ne.tooltip_style = osc_styles.Tooltip
    ne.tooltipF = function ()
        local msg = texts.off
        if not (get_track('sub') == 0) then
            msg = (texts.subtitle .. ' [' .. get_track('sub') .. ' ∕ ' .. #tracks_osc.sub .. '] ')
            local prop = mp.get_property('current-tracks/sub/lang')
            if not prop then
                prop = texts.na
            end
            msg = msg .. '[' .. prop .. ']'
            prop = mp.get_property('current-tracks/sub/title')
            if prop then
                msg = msg .. ' ' .. prop
            end
            return msg
        end
        return msg
    end
    ne.eventresponder['mbtn_left_up'] =
        function () set_track('sub', 1) end
    ne.eventresponder['mbtn_right_up'] =
        function () set_track('sub', -1) end
    ne.eventresponder['mbtn_mid_up'] =
        function () show_message(get_tracklist('sub')) end

    -- vol_ctrl
    ne = new_element('vol_ctrl', 'button')
    ne.enabled = (get_track('audio')>0)
    ne.visible = (osc_param.playresx >= 650) and user_opts.volumecontrol
    ne.content = function ()
        if (state.mute) then
            return ('\xEF\x8E\xBB')
        else
            return ('\xEF\x8E\xBC')
        end
    end
    ne.eventresponder['mbtn_left_up'] =
        function () mp.commandv('cycle', 'mute') end

    --tog_fs
    ne = new_element('tog_fs', 'button')
    ne.content = function ()
        if (state.fullscreen) then
            return ('\xEF\x85\xAC')
        else
            return ('\xEF\x85\xAD')
        end
    end
    ne.visible = (osc_param.playresx >= 540)
    ne.eventresponder['mbtn_left_up'] =
        function () mp.commandv('cycle', 'fullscreen') end

    --tog_info
    ne = new_element('tog_info', 'button')
    ne.content = ''
    ne.visible = (osc_param.playresx >= 600)
    ne.eventresponder['mbtn_left_up'] =
        function () mp.commandv('script-binding', 'stats/display-stats-toggle') end

    -- title
    ne = new_element('title', 'button')
    ne.content = function ()
        local title = mp.command_native({'expand-text', user_opts.title})
        title = title:gsub('\\n', ' '):gsub('\\$', ''):gsub('{','\\{')
        return not (title == '') and title or ' '
    end
    ne.visible = osc_param.playresy >= 320 and user_opts.showtitle

    -- episode tag line (e.g. "S02E05") shown below the title for series episodes
    ne = new_element('ep_subtitle', 'button')
    ne.content = function () return ep_subtitle end
    ne.visible = osc_param.playresy >= 320 and user_opts.showtitle and ep_subtitle ~= ''

    --seekbar
    ne = new_element('seekbar', 'slider')

    ne.enabled = not (mp.get_property('percent-pos') == nil)
    ne.slider.markerF = function ()
        local duration = mp.get_property_number('duration', nil)
        if not (duration == nil) then
            local chapters = mp.get_property_native('chapter-list', {})
            local markers = {}
            for n = 1, #chapters do
                markers[n] = (chapters[n].time / duration * 100)
            end
            return markers
        else
            return {}
        end
    end
    ne.slider.posF =
        function () return mp.get_property_number('percent-pos', nil) end
    ne.slider.tooltipF = function (pos)
        local duration = mp.get_property_number('duration', nil)
        if not ((duration == nil) or (pos == nil)) then
            local possec = duration * (pos / 100)
			local chapters = mp.get_property_native('chapter-list', {})
			if #chapters > 0 then
				local ch = #chapters
				local i
				for i = 1, #chapters do
					if chapters[i].time / duration * 100 >= pos then
						ch = i - 1
						break
					end
				end
				if ch == 0 then
					return string.format('[%s] [0/%d]', mp.format_time(possec), #chapters)
				elseif chapters[ch].title then
					return string.format('[%s] [%d/%d][%s]', mp.format_time(possec), ch, #chapters, chapters[ch].title)
				end
			end
            return mp.format_time(possec)
        else
            return ''
        end
    end
    ne.slider.seekRangesF = function()
        if not user_opts.seekrange then
            return nil
        end
        local cache_state = state.cache_state
        if not cache_state then
            return nil
        end
        local duration = mp.get_property_number('duration', nil)
        if (duration == nil) or duration <= 0 then
            return nil
        end
        local ranges = cache_state['seekable-ranges']
        if #ranges == 0 then
            return nil
        end
        local nranges = {}
        for _, range in pairs(ranges) do
            nranges[#nranges + 1] = {
                ['start'] = 100 * range['start'] / duration,
                ['end'] = 100 * range['end'] / duration,
            }
        end
        return nranges
    end
    ne.eventresponder['mouse_move'] = --keyframe seeking when mouse is dragged
        function (element)
            if not element.state.mbtnleft then return end -- allow drag for mbtnleft only!
            -- mouse move events may pile up during seeking and may still get
            -- sent when the user is done seeking, so we need to throw away
            -- identical seeks
            local seekto = get_slider_value(element)
            if (element.state.lastseek == nil) or
                (not (element.state.lastseek == seekto)) then
                    local flags = 'absolute-percent'
                    if not user_opts.seekbarkeyframes then
                        flags = flags .. '+exact'
                    end
                    mp.commandv('seek', seekto, flags)
                    element.state.lastseek = seekto
            end

        end
    ne.eventresponder['mbtn_left_down'] = --exact seeks on single clicks
        function (element)
            mp.commandv('seek', get_slider_value(element), 'absolute-percent', 'exact')
            element.state.mbtnleft = true
        end
    ne.eventresponder['mbtn_left_up'] =
        function (element) element.state.mbtnleft = false end
    ne.eventresponder['mbtn_right_down'] = --seeks to chapter start
        function (element)
            local duration = mp.get_property_number('duration', nil)
            if not (duration == nil) then
                local chapters = mp.get_property_native('chapter-list', {})
                if #chapters > 0 then
                    local pos = get_slider_value(element)
                    local ch = #chapters
                    for n = 1, ch do
                        if chapters[n].time / duration * 100 >= pos then
                            ch = n - 1
                            break
                        end
                    end
                    mp.commandv('set', 'chapter', ch - 1)
                    --if chapters[ch].title then show_message(chapters[ch].time) end
                end
            end
        end
    ne.eventresponder['reset'] =
        function (element) element.state.lastseek = nil end

    --volumebar
    ne = new_element('volumebar', 'slider')
    ne.visible = (osc_param.playresx >= 700) and user_opts.volumecontrol
    ne.enabled = (get_track('audio')>0)
    ne.slider.tooltipF =
		function (pos)
			local refpos = state.proc_volume
			if refpos > 100 then refpos = 100 end
			if pos+3 >= refpos and pos-3 <= refpos then
				return string.format('%d', state.proc_volume)
			else
				return ''
			end
		end
    ne.slider.markerF = nil
    ne.slider.seekRangesF = nil
    ne.slider.posF =
        function ()
            return state.proc_volume
        end
    ne.eventresponder['mouse_move'] =
        function (element)
            if not element.state.mbtnleft then return end
            local seekto = get_slider_value(element)
            if (element.state.lastseek == nil) or
                (not (element.state.lastseek == seekto)) then
                    set_volume(seekto)
                    element.state.lastseek = seekto
            end
        end
    ne.eventresponder['mbtn_left_down'] = --exact seeks on single clicks
        function (element)
            local seekto = get_slider_value(element)
            set_volume(seekto)
            element.state.mbtnleft = true
        end
    ne.eventresponder['mbtn_left_up'] =
        function (element)
			element.state.mbtnleft = false
		end
    ne.eventresponder['reset'] =
        function (element) element.state.lastseek = nil end
    ne.eventresponder['wheel_up_press'] =
        function (element)
			set_volume(state.proc_volume+5)
		end
    ne.eventresponder['wheel_down_press'] =
        function (element)
			set_volume(state.proc_volume-5)
		end
    -- tc_left (current pos)
    ne = new_element('tc_left', 'button')
    ne.content = function ()
        if (state.tc_ms) then
            return (mp.get_property_osd('playback-time/full'))
        else
            return (mp.get_property_osd('playback-time'))
        end
    end
    ne.eventresponder['mbtn_left_up'] = function ()
        state.tc_ms = not state.tc_ms
        request_init()
    end

    -- tc_right (total/remaining time)
    ne = new_element('tc_right', 'button')
    ne.content = function ()
        if (mp.get_property_number('duration', 0) <= 0) then return '--:--:--' end
        if (state.rightTC_trem) then
            if (state.tc_ms) then
                return ('-'..mp.get_property_osd('playtime-remaining/full'))
            else
                return ('-'..mp.get_property_osd('playtime-remaining'))
            end
        else
            if (state.tc_ms) then
                return (mp.get_property_osd('duration/full'))
            else
                return (mp.get_property_osd('duration'))
            end
        end
    end
    ne.eventresponder['mbtn_left_up'] =
        function () state.rightTC_trem = not state.rightTC_trem end

    -- load layout
    layouts()

    -- load window controls
    if window_controls_enabled() then
        window_controls()
    end

    --do something with the elements
    prepare_elements()
end
