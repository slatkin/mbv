# twitch-url-playback Specification

## Purpose

Lets the user paste a Twitch URL into the TUI and play it at once in the Owner queue, with mpv and yt-dlp resolving the stream.

## Requirements

### Requirement: A keybind action opens the Twitch URL prompt
mbv SHALL provide a configurable keybind action that opens a modal text prompt for a Twitch URL. The prompt SHALL hold the focus while it is open. Cancel SHALL close it and change nothing.

#### Scenario: User opens the prompt
- **WHEN** the user presses the chord bound to the Twitch URL action
- **THEN** a text prompt for a URL opens and receives the text input

#### Scenario: User cancels the prompt
- **WHEN** the prompt is open and the user cancels it
- **THEN** the prompt closes and the queue and playback do not change

### Requirement: The prompt accepts only Twitch hosts
The prompt SHALL accept a URL only when its scheme is `http` or `https` and its host is `twitch.tv` or a subdomain of `twitch.tv`. It SHALL NOT examine the path. On refusal, the prompt SHALL stay open with the input kept and SHALL show that the URL is not a Twitch URL.

#### Scenario: Live channel URL passes
- **WHEN** the user submits `https://www.twitch.tv/somechannel`
- **THEN** mbv accepts it

#### Scenario: VOD and clip URLs pass
- **WHEN** the user submits `https://www.twitch.tv/videos/123456` or `https://clips.twitch.tv/SomeSlug`
- **THEN** mbv accepts it

#### Scenario: Other host is refused
- **WHEN** the user submits `https://www.youtube.com/watch?v=abc`
- **THEN** mbv refuses it, the prompt stays open with the input kept, and nothing enters the queue

#### Scenario: Look-alike host is refused
- **WHEN** the user submits `https://nottwitch.tv/somechannel`
- **THEN** mbv refuses it

### Requirement: Submit plays the URL at once as a Twitch item
On an accepted URL, mbv SHALL close the prompt and submit a Twitch item through the same play-now path as other items. The Owner SHALL hold it as one slot in the canonical queue. Pause, play, stop, next, previous, the queue panel, and MPRIS SHALL act on it as on any other slot.

#### Scenario: Accepted URL starts playback
- **WHEN** the user submits an accepted Twitch URL
- **THEN** the prompt closes, the Twitch item becomes the active slot of the Owner queue, and playback starts

#### Scenario: Transport controls act on the Twitch item
- **WHEN** a Twitch item plays and the user pauses through mbv keys or MPRIS
- **THEN** playback of the Twitch item pauses

### Requirement: A Twitch item carries a title derived from its URL
A Twitch item SHALL carry the URL as given and a title derived from that URL. Queue and playback surfaces SHALL show that title.

#### Scenario: Queue shows the derived title
- **WHEN** a Twitch item for `https://www.twitch.tv/somechannel` is in the queue
- **THEN** its queue row names it by a title derived from that URL

### Requirement: mpv receives the Twitch URL unchanged
The Player owner SHALL give the Twitch item URL to mpv exactly as the user gave it. mbv SHALL NOT resolve, rewrite, or select a stream for it. Duration and seek SHALL follow what mpv reports for the loaded stream.

#### Scenario: mpv loads the given URL
- **WHEN** the Player owner starts a Twitch item
- **THEN** mpv is asked to load the exact URL of that item

#### Scenario: Stream does not resolve
- **WHEN** mpv fails to load the Twitch URL (for example, the channel is offline)
- **THEN** mbv treats it as a failed load of that slot, the same as a failed load of any other item

### Requirement: A Twitch item keeps no playback state
A Twitch item SHALL have no resume position and no played state. A restart of a Twitch item SHALL begin where mpv begins the stream.

#### Scenario: Replay after stop
- **WHEN** the user stops a Twitch item and later plays the same slot again
- **THEN** playback starts with no stored position

### Requirement: Only mpv output plays a Twitch item
A Cast output and an Emby Session output SHALL refuse a Twitch item and name the item in the refusal. An audio-only Player owner SHALL refuse a Twitch item as a video item.

#### Scenario: Cast output active
- **WHEN** the Owner output is a Cast receiver and a Twitch item becomes active
- **THEN** the Cast output refuses it as uncastable and names the item

#### Scenario: Emby Session output active
- **WHEN** the Owner output is an Emby Session and a Twitch item becomes active
- **THEN** the Session output refuses it and names the item

#### Scenario: Audio-only owner
- **WHEN** a Twitch item is submitted to an audio-only Player owner
- **THEN** owner admission refuses it

### Requirement: A Twitch item persists with the queue
A Twitch item SHALL persist and restore with the Owner queue like other items.

#### Scenario: Owner restart restores the slot
- **WHEN** the Owner queue holds a Twitch item and the Owner process restarts
- **THEN** the restored queue holds the same Twitch item with the same URL and title
