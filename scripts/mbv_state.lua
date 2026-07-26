-- by maoiscat
-- email:valarmor@163.com
-- https://github.com/maoiscat/mpv-osc-morden

local assdraw = require 'mp.assdraw'
local msg = require 'mp.msg'
local opt = require 'mp.options'
local utils = require 'mp.utils'

--
-- Parameters
--
-- default user option values
-- may change them in osc.conf
local user_opts = {
    showwindowed = true,        -- show OSC when windowed?
    showfullscreen = true,      -- show OSC when fullscreen?
    scalewindowed = 1,          -- scaling of the controller when windowed
    scalefullscreen = 1,        -- scaling of the controller when fullscreen
    scaleforcedwindow = 2,      -- scaling when rendered on a forced window
    vidscale = false,           -- scale the controller with the video?
    hidetimeout = 5000,         -- duration in ms until the OSC hides if no
                                -- mouse movement. enforced non-negative for the
                                -- user, but internally negative is 'always-on'.
    fadeduration = 500,         -- duration of fade out in ms, 0 = no fade
    minmousemove = 3,           -- minimum amount of pixels the mouse has to
                                -- move between ticks to make the OSC show up
    iamaprogrammer = false,     -- use native mpv values and disable OSC
                                -- internal track list management (and some
                                -- functions that depend on it)
    font = 'mpv-osd-symbols',    -- default osc font
    seekrange = true,            -- show seekrange overlay
    seekrangealpha = 128,          -- transparency of seekranges
    seekbarkeyframes = true,    -- use keyframes when dragging the seekbar
    title = '${media-title}',   -- string compatible with property-expansion
                                -- to be shown as OSC title
    showtitle = true,            -- show title and no hide timeout on pause
    timetotal = true,              -- display total time instead of remaining time?
    timems = false,             -- display timecodes with milliseconds
    visibility = 'auto',        -- only used at init to set visibility_mode(...)
    windowcontrols = 'auto',    -- whether to show window controls
    volumecontrol = true,       -- whether to show mute button and volumne slider
    processvolume = false,		-- disabled: mbv Rust side handles volume sqrt scaling
    language = 'eng',            -- eng=English, chs=Chinese
}

-- Localization
local language = {
    ['eng'] = {
        welcome = '{\\fs24\\1c&H0&\\3c&HFFFFFF&}Drop files or URLs to play here.',  -- this text appears when mpv starts
        off = 'OFF',
        na = 'n/a',
        none = 'none',
        video = 'Video',
        audio = 'Audio',
        subtitle = 'Subtitle',
        available = 'Available ',
        track = ' Tracks:',
        playlist = 'Playlist',
        nolist = 'Empty playlist.',
        chapter = 'Chapter',
        nochapter = 'No chapters.',
    },
    ['chs'] = {
        welcome = '{\\1c&H00\\bord0\\fs30\\fn微软雅黑 light\\fscx125}MPV{\\fscx100} 播放器',  -- this text appears when mpv starts
        off = '关闭',
        na = 'n/a',
        none = '无',
        video = '视频',
        audio = '音频',
        subtitle = '字幕',
        available = '可选',
        track = '：',
        playlist = '播放列表',
        nolist = '无列表信息',
        chapter = '章节',
        nochapter = '无章节信息',
    }
}
-- read options from config and command-line
opt.read_options(user_opts, 'osc', function(list) update_options(list) end)
-- apply lang opts
local texts = language[user_opts.language]
local osc_param = { -- calculated by osc_init()
    playresy = 0,                           -- canvas size Y
    playresx = 0,                           -- canvas size X
    display_aspect = 1,
    unscaled_y = 0,
    areas = {},
}

local osc_styles = {
    TransBg = '{\\blur100\\bord140\\1c&H000000&\\3c&H000000&}',
    SeekbarBg = '{\\blur0\\bord0\\1c&H555555&}',
    SeekbarFg = '{\\blur1\\bord1\\1c&H4BB552&}',
    VolumebarBg = '{\\blur0\\bord0\\1c&H555555&}',
    VolumebarFg = '{\\blur1\\bord1\\1c&H4BB552&}',
    Ctrl1 = '{\\blur0\\bord0\\1c&HFFFFFF&\\3c&HFFFFFF&\\fs36\\fnmaterial-design-iconic-font}',
    Ctrl2 = '{\\blur0\\bord0\\1c&HFFFFFF&\\3c&HFFFFFF&\\fs24\\fnmaterial-design-iconic-font}',
    Ctrl3 = '{\\blur0\\bord0\\1c&HFFFFFF&\\3c&HFFFFFF&\\fs24\\fnmaterial-design-iconic-font}',
    Time = '{\\blur0\\bord0\\1c&HFFFFFF&\\3c&H000000&\\fs17\\fn' .. user_opts.font .. '}',
    Tooltip = '{\\blur1\\bord0.5\\1c&HFFFFFF&\\3c&H000000&\\fs18\\fn' .. user_opts.font .. '}',
    Title = '{\\blur1\\bord0.5\\1c&HFFFFFF&\\3c&H0\\fs48\\q2\\fn' .. user_opts.font .. '}',
    WinCtrl = '{\\blur1\\bord0.5\\1c&HFFFFFF&\\3c&H0\\fs20\\fnmpv-osd-symbols}',
    elementDown = '{\\1c&H999999&}',
}

-- episode subtitle tag (e.g. "S02E05") set via mbv-ep-info script-message
local ep_subtitle = ''

-- internal states, do not touch
local state = {
    showtime,                               -- time of last invocation (last mouse move)
    osc_visible = false,
    anistart,                               -- time when the animation started
    anitype,                                -- current type of animation
    animation,                              -- current animation alpha
    mouse_down_counter = 0,                 -- used for softrepeat
    active_element = nil,                   -- nil = none, 0 = background, 1+ = see elements[]
    active_event_source = nil,              -- the 'button' that issued the current event
    rightTC_trem = not user_opts.timetotal, -- if the right timecode should display total or remaining time
    tc_ms = user_opts.timems,               -- should the timecodes display their time with milliseconds
    mp_screen_sizeX, mp_screen_sizeY,       -- last screen-resolution, to detect resolution changes to issue reINITs
    initREQ = false,                        -- is a re-init request pending?
    last_mouseX, last_mouseY,               -- last mouse position, to detect significant mouse movement
    mouse_in_window = false,
    message_text,
    message_hide_timer,
    fullscreen = false,
    tick_timer = nil,
    tick_last_time = 0,                     -- when the last tick() was run
    hide_timer = nil,
    cache_state = nil,
    idle = false,
    enabled = true,
    input_enabled = true,
    showhide_enabled = false,
    dmx_cache = 0,
    border = true,
    maximized = false,
    osd = mp.create_osd_overlay('ass-events'),
    mute = false,
    lastvisibility = user_opts.visibility,		-- save last visibility on pause if showtitle
    sys_volume,									--system volume
    proc_volume,								--processed volume
}

local window_control_box_width = 138
local tick_delay = 0.03

--
-- Helperfunctions
--
