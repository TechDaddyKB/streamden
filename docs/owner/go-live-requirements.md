# Go-live requirements: what you must do to be able to stream on each platform

Last verified against official docs where possible: 2026-10-18. Part of the [owner runbooks](README.md). This is separate from developer-app registration: to **test StreamDen against a real platform** you need to be able to go live there (or to fake it safely). Several requirements have **waiting periods or follower thresholds**, so start the slow ones early (see "Start these clocks now").

Source quality: where an official page could not be fetched (X, TikTok, Kick, Rumble, YouTube activation time) I used third-party guides and say so. Always trust the screen over this file.

## Start these clocks now

| Item | Why it is slow |
|---|---|
| YouTube live-streaming activation | One-time wait, reported as up to 24 hours (see YouTube). |
| Twitch Affiliate | Needed for Channel Points, subs and Bits on your test channel. Thresholds were reportedly lowered in 2025-2026 and sources disagree (older guides: 50 followers, 500 minutes, 7 days, 3 average viewers; newer: 25 followers, 4 hours, 4 days, 3 average viewers). Check your **Creator Dashboard > Achievements** for the real numbers. |
| Facebook | Account at least 60 days old, and Pages/professional profiles at least 100 followers. Cannot be rushed. |
| TikTok LIVE | Commonly 1,000 followers and age 18+. Cannot be rushed. |
| X Live | Needs an X Premium subscription (monthly cost). Subscribe only when you test. |
| Kick | Verification/payout KYC takes 24-72 hours (monetization only; not needed to stream). |

## General RTMP / OBS notes (all platforms)

1. In OBS: **Settings > Stream**. Choose the platform in **Service** if it is listed (Twitch and YouTube can also link your account), or **Custom...** and paste the **Server** (ingest URL) and **Stream Key** from the platform.
2. **A stream key is a password.** Anyone with it can broadcast on your channel. Never commit it, never show OBS Settings > Stream on stream, and reset the key if it ever appears in a screenshot or log. OBS stores keys in plain text inside your profile folder, so do not commit or share OBS profile exports.
3. Recommended starting settings: H.264 (NVENC on an NVIDIA RTX card is fine), CBR, keyframe interval 2 s (X recommends 3 s), AAC audio 128 kbps. Keep bitrate within each platform's maximum (X: 12 Mbps max recommended; check each platform).
4. Each platform takes **one ingest at a time per key**. To stream to several platforms at once you need a multi-output plugin or a restreaming service (out of scope here).
5. For Wayland/Hyprland capture, OBS uses PipeWire/portal capture. This is unrelated to platform eligibility.
6. Write stream keys into the password manager, not into `.env.local`. StreamDen itself does not need your stream key; it controls OBS over obs-websocket.

## Safe testing without an audience

- **Use a second account as the viewer** (another browser profile) to see chat and generate events (follow, chat, subscribe-like actions you can do yourself).
- **Use private or unlisted visibility where it exists** (YouTube, possibly Rumble, Facebook audience "Only me"; see per platform).
- **Use fake events instead of real ones.** Twitch CLI can mock EventSub (below). Streamlabs/StreamElements/Ko-fi/Patreon have test-alert buttons. Keep recorded JSON fixtures of real events for regression tests.
- **Local ingest for encoder tests**: run a local RTMP server (for example MediaMTX in a container) and point OBS at `rtmp://127.0.0.1/live` to test encoder settings without touching any platform.
- **Name tests clearly** (title "Test stream, ignore") and use a low-traffic category so accidental viewers are not misled.
- **Don't test with real money** unless you can refund it. Tips and subscriptions to yourself still incur fees.

---

## Twitch

**Requirements to stream**

- A Twitch account with a verified email and **two-factor authentication enabled**. Twitch refuses to let you stream otherwise (reported error code `788078D4`, "Two-factor authentication is required to stream"; community and forum sources). Set it up at <https://www.twitch.tv/settings/security>.
- Nothing else. No follower minimum to stream.

**Stream key / server**

- **Creator Dashboard > Settings > Stream > Primary Stream Key** (click **Show**; **Reset** generates a new key). Desktop browser only; the mobile app does not show it. In OBS pick **Twitch** as the service and link your account, or paste the key under **Custom**.

**Safe testing**

- **Twitch CLI mock server** (official): no stream needed. `twitch event websocket start-server` (listens at `ws://127.0.0.1:8080/ws`, port changeable with `-p`), then in another terminal `twitch event trigger channel.ban --transport=websocket` (or any EventSub type). Install via Homebrew (`brew install twitchdev/twitch/twitch-cli`), Scoop, or the GitHub release binary (AUR availability is not documented, verify). Also `twitch event websocket reconnect` to test the reconnect flow and `twitch mock-api` for a mock Helix.
- **Bandwidth test mode**: appending `?bandwidthtest=true` to the stream key (or OBS's bandwidth-test option when available) sends a real stream that does not go live. Community-documented, not on an official page; confirm your channel stays offline.
- Real channel with "Test stream" title, a second account in chat.

**What you cannot test without eligibility**: Channel Points rewards and subscriptions/Bits need Affiliate or Partner. Test those with the CLI mock.

## YouTube

**Requirements to stream**

- A **verified channel** (phone verification in YouTube Studio under **Settings > Channel > Feature eligibility**, per secondary guides; verify on screen).
- **No live-streaming restrictions on the channel in the past 90 days.**
- Age: you must be at least 16 to live stream.
- Enable it: **YouTube Studio > Create > Go live**. The first time starts a **one-time activation, reported as up to 24 hours** (third-party guides; YouTube's official page does not state the delay). You cannot bypass it.
- Mobile live needs additional requirements (previously 50 subscribers; unconfirmed). Not relevant to encoder streaming from OBS.
- Monetization features (Super Chat, memberships, Super Stickers, Jewels gifts) require the **YouTube Partner Program** and cannot be produced on a normal test channel. Use recorded fixtures for those events.

**Stream key / server**

- **YouTube Studio > Create > Go live > Stream** tab (encoder) > **Stream key** (and Server URL). You can create more than one stream key. Limits: 10 active streams per channel, 3 per stream key.

**Safe testing**

- Set **Visibility** to **Unlisted** on the stream setup (privacy is set on YouTube's side and carries into OBS/RTMP). Unlisted is link-gated: only people with the URL can watch, and live chat works for signed-in Google users (verify in your channel). **Private** streams are invitation-only and a guide reports comments are unavailable, so test chat on Unlisted.
- API test that costs almost nothing: `liveBroadcasts.list` (1 unit) to get the `liveChatId`, then `liveChatMessages.streamList` (5 units) while you chat from a second account.
- Watch the quota page while you test ([google-youtube.md](registrations/google-youtube.md)).

## Kick

**Requirements to stream**

- A Kick account with a verified email. Turn on **2FA** (required for developer tools anyway). Secondary guides say you can stream with just an email, but **monetization, payouts and the verified badge** need phone verification and KYC, and some features need an Affiliate/verified status. Kick's affiliate rules have changed over time (older: follower and hours thresholds; newer: mostly auto-eligible after KYC), so check Kick's own help pages.

**Stream key / server**

- **Creator Dashboard** > the stream key area (guides name **Settings > Stream Key**, **Channel > Stream URL and Key**, or the page `kick.com/dashboard/stream`; verify on screen). Kick uses RTMPS with an account-specific ingest URL. Use the **exact URL shown on screen**; encoder guides disagree on whether a suffix such as `/app` is needed (vMix says `:443/app`, StreamYard `/app`, OneStream `/live`), so try the dashboard value first.

**Safe testing**

- Kick has no unlisted/private broadcast mode that we could verify. Options: stream to a **second test channel** (a spare Kick account), use a clear "TEST" title and a quiet category, or test the API without streaming at all (chat send works only on a live channel, but follows, subscriptions and webhooks can be tested with a second account).
- Kick's docs UI lets you try API endpoints without hosting an app (set the redirect to `https://docs.kick.com`).

## Rumble

**Requirements to stream**

- Rumble accounts must be able to go live. Guides disagree: older ones say you need at least 5 followers, a paid subscription or a verified account; a newer one (Castr) says live streaming is open to all users, sometimes after **phone verification**. Check on your account.

**Stream key / server**

- Use **Rumble Studio > Direct RTMP**: create a Direct RTMP room (only one room open at a time), fill in the stream details, then copy the **stream URL** and **stream key** (older guides show `rtmp://rtmp.rumble.com/live`; use what the room shows). Direct RTMP uses a **static key** you can reset at any time. Go live only after the room shows "Connected to RTMP stream".
- The **Live Stream API URL** (for StreamDen) is separate: see [rumble.md](registrations/rumble.md).

**Safe testing**

- Rumble's stream setup has visibility options (verify on screen whether unlisted/private exist for live). Otherwise use a test title and a second account. The Live Stream API returns real data only while live.

## X Live

**Requirements to stream (RTMP / external encoder)**

- **X Premium** is required for **Live Studio** (the successor to Media Studio Producer, launched in beta in July 2026). Source: third-party coverage (streamer.guide, upstream.so); X's help center page returned 403 to automated fetching, so confirm on screen. Non-Premium accounts keep standard in-app X Live (mobile).
- Your account must be **public**: protected posts block going live.

**Steps and stream key**

1. Open <https://x.com/i/live-studio> (or Creator Studio > Live Studio).
2. **New Livestream** > enter a title > create a **source** > choose the **ingest region** closest to you (it cannot be changed later).
3. Copy the **server URL** and **stream key** into OBS (**Service: Custom**, leave authentication off). Find them later under **Manage Sources**. An RTMPS URL is also offered. Treat the key like a password. Up to 100 sources per account, one stream at a time per source.
4. Upload a 16:9 thumbnail, choose **Now** or **Later** (schedule, auto-start), set visibility, chat permissions (Everyone, Verified, Accounts You Follow, Subscribers, or off) and any country restrictions (visibility and country limits lock once the stream starts), write the announcement post, press **Go Live**.

**Limits**: each stream ends after 24 hours; a timed-out stream cannot be restarted; recommended 1080p60 at 12 Mbps, H.264/AAC, 128 kbps audio, keyframe about every 3 seconds (maximum 4K60 at 40 Mbps).

**Safe testing**: restrict chat and set the narrowest audience the visibility options allow (options: verify on screen), keep the test short, or skip live tests and test only the **posting** feature (cost: $0.015, or $0.20 with a link, see [x-twitter.md](registrations/x-twitter.md)). Reading X Live **chat** through the API is not available to us (whitelisted Livestream API).

## TikTok LIVE

**Requirements to stream**

- Sources agree on roughly: **1,000 followers**, **age 18+** (one says 16 may go live but cannot receive gifts; unconfirmed), a public account in good standing with no active Community Guidelines violations, and a personal rather than business account for gifts. A few users report access with fewer followers (anecdotal). TikTok's official help center could not be fetched, so these come from third-party guides (TikFinity, OneStream, others). Verify in the TikTok app.
- **Stream key access** (to use OBS/RTMP instead of the phone): after you have LIVE access, log in on a desktop browser, open <https://livecenter.tiktok.com> (or **Go LIVE** from the left sidebar), open the LIVE Room producer, choose a title and category, **Save & Go LIVE**; the **Server URL** and **Stream Key** appear (guides say once you are live). Not every eligible account sees the key. **TikTok LIVE Studio** is reported as **Windows-only**, so on Linux (your Omarchy machine) the RTMP key route is the only way. Reports of "stream key agencies" and keys that reset per session exist (unverified); treat the key as single-session.

**Safe testing**

- TikTok has no verifiable private/test LIVE mode. Streaming means going public.
- You do **not** have to stream to develop the (unofficial, opt-in) TikTok connector: it reads any **public** LIVE room by username. Watch someone else's live (respect their privacy and TikTok's terms) with a throwaway account. See [tiktok.md](registrations/tiktok.md).

## Facebook

**Requirements to stream**

- A **Facebook account at least 60 days old**; a **Page or professional-mode profile needs at least 100 followers** to go live (Meta developer docs and third-party summaries; personal profiles are described inconsistently, verify on screen). Page broadcasts need Facebook access or task access that lets you create content.

**Stream key / server**

- **Facebook Live Producer** (<https://www.facebook.com/live/producer>): choose **Streaming software** as the video source; it shows a **Server URL** and **Stream Key**. **Reset Stream Key** generates a new one. Paste into OBS (Custom service), start streaming in OBS, then click **Go Live** in Live Producer.

**Safe testing**

- Set the **audience to Only me** (privacy control in Live Producer; verify on screen) before going live. The Live Video API also supports unpublished test broadcasts (verify in the Live Video API docs before relying on it). Comments/SSE testing needs a Page-level app with Standard Access (your own roles) — see [facebook.md](registrations/facebook.md).

---

## Per-platform summary

| Platform | Account gate | Stream key location | Private/test option | Slow step |
|---|---|---|---|---|
| Twitch | email + 2FA | Creator Dashboard > Settings > Stream | CLI mock; `?bandwidthtest=true`; test title | Affiliate for points/subs |
| YouTube | phone-verified channel, no live restriction (90 d), 16+ | Studio > Create > Go live > Stream | **Unlisted** (Private = no chat) | up to 24 h activation |
| Kick | email, 2FA | Creator Dashboard (stream key page) | none verified: second account | KYC for payouts |
| Rumble | account (maybe phone verification) | Rumble Studio > Direct RTMP | check visibility on screen | unclear eligibility |
| X Live | **Premium**, public account | x.com/i/live-studio > source | narrow audience/chat; or test posting only | Premium subscription |
| TikTok LIVE | ~1,000 followers, 18+ | livecenter.tiktok.com (after access) | none; read others' public LIVE for connector dev | follower count |
| Facebook | account 60 d, Page 100 followers | Live Producer > Streaming software | audience "Only me" | age and follower gates |

## Sources

- Twitch stream key and 2FA error (community): <https://discuss.dev.twitch.tv/t/streaming-key-problem/22059>; 2FA setup: <https://www.twitch.tv/settings/security>
- Twitch CLI: <https://dev.twitch.tv/docs/cli/> and <https://dev.twitch.tv/docs/cli/websocket-event-command/>
- Twitch bandwidth test (community): <https://github.com/danieloneill/TwitchTest>
- Twitch Affiliate (conflicting guides): <https://streamer.guide/guides/twitch-affiliate-requirements>
- YouTube live eligibility: <https://support.google.com/youtube/answer/2474026>
- YouTube stream key and limits: <https://support.google.com/youtube/answer/9227509>
- Kick app and 2FA: <https://docs.kick.com/getting-started/kick-apps-setup.md>; stream key guides (secondary): <https://castr.com/blog/how-to-find-kick-stream-key/>, <https://support.streamyard.com/hc/en-us/articles/16599000591508-Streaming-to-Kick-com>
- Rumble Direct RTMP: <https://rumble.support/help/how-to-use-rumble-studios-direct-rtmp-feature>
- X Live Studio (secondary): <https://streamer.guide/blog/x-live-studio-update-july-2026>, <https://upstream.so/blog/how-to-go-live-on-x/>; X help: <https://help.x.com/en/using-x/live-studio> (403 to automated fetch)
- TikTok LIVE (secondary): <https://blog.tikfinity.com/how-to-stream-on-tiktok-with-obs-2025-guide/>, <https://onestream.live/blog/how-to-get-live-access-on-tiktok/>
- Facebook Live Video API: <https://developers.facebook.com/documentation/live-video-api/getting-started>
