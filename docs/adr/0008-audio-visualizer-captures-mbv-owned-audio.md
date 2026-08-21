# Audio visualizer captures the audible system output

The embedded visualizer owns one PipeWire capture stream connected to the current default system-output monitor. It receives converted interleaved stereo PCM and renders it in the client that enabled visualization. A same-host Local daemon's playback is therefore capturable by each attached Client without daemon or ctrl-protocol changes.

mbv does not create, modify, or destroy persistent PipeWire sinks, sources, links, loopbacks, or modules. It also does not change mpv's audio output properties. Starting or stopping the visualizer must leave playback configuration and the rest of the audio graph unchanged.

PipeWire failure is isolated from playback: the worker reports the diagnostic, clears the vectorscope, and stops without interrupting player control or input handling.
