//! Contract: spec `twitch-url-playback`, "mpv receives the Twitch URL
//! unchanged" — the Player owner gives the item's URL to mpv exactly as the
//! user gave it (design D3).

use mbv_queue::TwitchItem;

#[test]
fn twitch_url_reaches_mpv_unchanged() {
    let url = "https://twitch.tv/somechannel";
    let item = TwitchItem::from_url(url).expect("a twitch.tv URL parses");
    let built = super::mpv_url_for_queue_item(
        super::MpvUrlSource::Twitch(&item),
        "http://emby.example:8096",
        "token",
    );
    assert_eq!(built, url);
}
