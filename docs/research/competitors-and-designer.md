# Competitor Scan & WYSIWYG Overlay/Alert Designer Research

**Date:** 2026-10-18
**Scope:** (A) competitor and prior-art scan for Streamer.live, the unified Streamer.bot + Speaker.bot + SAMMI replacement. (B) Prior art and a proposed design for the WYSIWYG designer used for chat overlays, alerts and widgets, plus the requirements for multichat dock and overlay filtering.
**Related:** `docs/research/platform-apis.md` covers platform API details in more depth.

> **Method and confidence.** Sources are cited inline. The web-search budget ran out partway through the research. Claims taken from prior knowledge that could not be re-checked are marked **(verify)**. Library versions and licenses come straight from the npm registry on 2026-10-18 (`registry.npmjs.org/<pkg>`).

---

## TL;DR

1. **No competitor combines everything we plan to build.** Firebot, Mix It Up and Lumia combine automation with overlays. Social Stream Ninja (SSN) is the reference for multichat breadth. StreamElements and Streamlabs set the UX conventions for alert editors: variations, conditions, the queue and test-fire. Streamer.bot has a chat client and dock, but its overlay is experimental and has **no alert or overlay designer** ([SB Chat docs](https://docs.streamer.bot/guide/extra-features/chat)).
2. **Main design idea: one template model for alerts and chat rows.** An alert and a chat row are both a *template* (a small composition) with *conditional variations*, *enter/idle/exit animations* and *variable bindings*. One designer, one renderer and one condition engine can serve alerts, chat rows, event lists and widgets.
3. **Rendering: use DOM/CSS, not canvas,** in both the editor and the OBS runtime, sharing one renderer package. That gives a true WYSIWYG preview, crisp text, emotes rendered as `<img>`, custom HTML widgets and CSS/WAAPI animation. Canvas (PixiJS) is only for particle-style elements.
4. **Libraries:** Moveable + Selecto + @scena/guides for transform handles (MIT, but **unmaintained since Dec 2023**, so pin and vendor them). interact.js (MIT, active) is the fallback. Use WAAPI with Motion (MIT) for animation and dotLottie-web (MIT) for Lottie. Zod 4 for the schema. **Avoid** GSAP (license clause against no-code visual animation builders), tldraw (license key and watermark) and page builders such as GrapesJS, Craft.js and Puck (flow layout, not a broadcast canvas).

---

# Part A: Competitor / Prior-Art Scan

## A.1 Summary matrix

| Product | Category | Tech stack | OS | License / pricing | Platforms | Standout |
|---|---|---|---|---|---|---|
| **Firebot** | Desktop bot, automation, overlays | Electron 39, AngularJS 1.8 + Vue 3 + motion-v, TypeScript, Express 5, ws ([package.json](https://raw.githubusercontent.com/crowbartools/Firebot/v5/package.json)) | Win, macOS (Apple silicon only from v5.67), Linux **(verify Linux packaging)** | GPL-3.0, free ([repo](https://github.com/crowbartools/Firebot)) | Twitch-first. Kick only through a 3rd-party plugin ([Kick plugin](https://tomevault.io/tome/TheStaticMage/firebot-mage-kick-integration)) | Overlay widgets, Overlay Alert effect, Control Decks over phone QR, Plugin Manager ([releases](https://github.com/crowbartools/Firebot/releases)) |
| **Lumia Stream** | Desktop automation, lights, overlays | Electron **(verify)** | Win, Intel Mac, Apple Silicon Mac, Linux ([compare](https://www.lumiastream.com/compare)) | Free, or Premium at $5.99/mo, $32.99/6 mo, $59.99/yr, plus lifetime ([pricing](https://lumiastream.com/pricing)) | Twitch, YouTube, Kick, TikTok, Facebook, Trovo | Smart lights and "Fuze" audio/video sync, overlay editor, custom overlay code API, HUD mode |
| **Mix It Up** | Desktop bot, overlays | C#/.NET WPF **(verify)** | Windows 10/11 only ([site](https://mixitup.bot/)) | Free, open source on GitHub **(verify license, believed MIT)** | Twitch, YouTube, Kick, Velora, VPZone ([site](https://mixitup.bot/)) | 19 overlay widget types, multiple overlay endpoints, single-widget URLs ([overlay docs](https://mixitup.bot/docs/overlays)) |
| **Social Stream Ninja** | Multichat + overlays + automation | HTML/JS/CSS. Chrome/Firefox extension plus Electron standalone app ([repo](https://github.com/steveseguin/social_stream)) | Any (extension). Win/mac/Linux (app) | GPL-3.0, free | 120+ sites ([repo](https://github.com/steveseguin/social_stream)) | DOM capture plus WebSocket/API modes, about 100 URL-param filters, featured message, Event Flow, local AI TTS |
| **Touch Portal** | Remote button deck | Native host, Android/iOS clients | Win, macOS, Linux (beta AppImage) ([site](https://www.touch-portal.com/)) | Free. Pro is $13.99 one-time; plugins require Pro | via plugins (incl. Streamer.bot, Firebot, Kick) | Button logic (toggle/radio), Values, Events |
| **Aitum** | Rules automation + OBS plugins | Desktop app; OBS plugins in C++ | Multistream plugin: Win/mac/Linux, OBS ≥30 ([OBS forum](https://obsproject.com/forum/resources/aitum-multistream.1991/)) | Vertical plugin is free. App and Multistream pricing **(verify)** | Twitch, YouTube | Trigger→action rules, multi-PC, vertical canvas ([directory](https://contentcreators.com/tools/aitum)) |
| **Kruiz Control** | Browser-source automation | JS running inside an OBS browser source | Any (OBS) | CC BY-NC-ND 4.0, so **no code reuse** ([repo](https://github.com/Kruiser8/Kruiz-Control)) | Twitch, SE/SL alerts, OBS | Text-file pseudo-code triggers. Queued and parallel trigger files |
| **Nightbot / Fossabot** | Cloud chat bots | Cloud | n/a | Free | Nightbot: Twitch, YouTube. Fossabot adds Kick **(verify)** ([gyre](https://gyre.pro/blog/best-chatbot-to-use-for-streaming)) | Fossabot: configurable spam filters, audit log, RBAC |
| **Chatterino / Chatterino7** | Native chat client | C++/Qt | Win, Linux, macOS, FreeBSD ([site](https://chatterino.com/)) | MIT ([repo](https://github.com/Chatterino/chatterino2)) | Twitch | Splits, filter language, highlights, /mentions, streamer mode, 7TV paints (C7 v7.5.4, [MyNixOS](https://mynixos.com/nixpkgs/package/chatterino7)) |
| **Restream Chat** | Cloud multichat | Cloud, desktop, OBS dock | Any | Part of Restream plans | Restream destinations | Reply to all destinations, embed URL for overlay ([blog](https://restream.io/blog/restream-chat-everything-you-need-to-know/)) |
| **StreamElements** | Cloud overlays, alerts, bot | Cloud web editor | Any | Free | Twitch, YouTube, Kick **(verify Kick)** | Overlay editor, AlertBox, custom widgets with fields JSON, `SE_API.store` ([docs](https://docs.streamelements.com/overlays/getting-started)) |
| **Streamlabs** | Cloud widgets + Streamlabs Desktop | Cloud web + Electron desktop | Win/mac | Free + Ultra **(verify price)** | Twitch, YouTube, Kick, etc. | Alert Box variations with a rich condition list, alert moderation, profanity filter ([variations](https://streamlabs.com/content-hub/post/how-do-alert-box-variations-work)) |
| **OWN3D Pro** | Design marketplace + widgets | Cloud | Any | Core tools free. Stream Pass is quarterly ([pro](https://own3d.tv/en/pro)) | Twitch, YouTube | 12,000 designs in an Overlay Maker, about 500 alert designs, AI credits |
| **Overlay Expert** | Cloud overlay builder + Twitch Extension | Cloud | Any | **(verify)** | Twitch | 60+ animations. Panels that swap, scroll, peek and hide. Renders as a Twitch Extension, so it works for console streamers ([devpost](https://devpost.com/software/overlay-expert-axz1cs)) |
| **1UP Vision** (bonus find) | Cloud overlay editor | Web, React SDK | Any | Free/Basic/Pro, about $10–50/mo ([AlternativeTo](https://alternativeto.net/software/1up-vision/about/)) | Twitch, YouTube | 60+ layer types, keyframe alerts, sandboxed custom widgets with **SE import**, mod co-editing ([docs](https://docs.1up.vision/custom-widgets)) |
| **Botrix** | Cloud bot (Kick-focused) | Cloud | Any | Free (per 3rd party) ([eklipse](https://blog.eklipse.gg/tools/top-5-kick-stream-extension.html)) | Kick (+Twitch) | Commands, automod, points, song requests, alert widget URL |
| **Sound Alerts** | Twitch extension for paid triggers | Cloud | Any | Premium $6.99/mo ([premium](https://soundalerts.com/premium)) | Twitch | Viewer-triggered sound/video/TTS. Coins currency (Aug 2026), fullscreen effects (Aug 2026) ([changelog](https://feedback.soundalerts.com/changelog)) |
| **veadotube mini** | PNGtuber | Native | Win/mac/Linux **(verify)** | Paid on itch **(verify)** | n/a | WebSocket "nodes" API for states and push-to-talk ([API](https://veado.tube/docs/tech/api/)) |
| **VTube Studio** | VTuber (Live2D) | Unity | Win/mac, iOS/Android tracking | Free + DLC | n/a | Public WebSocket API with permission prompts, hotkeys, items, events ([wiki](https://github.com/DenchiSoft/VTubeStudio/wiki/Twitch-Hotkey-Triggers)) |
| *Streamer.bot (baseline)* | Desktop automation | C#/.NET | Windows | Free | Twitch, YouTube, Kick | Multichat client and dock with an experimental chat overlay. **No alert/overlay designer** ([docs](https://docs.streamer.bot/guide/extra-features/chat)) |

## A.2 Per-product notes

### Firebot (crowbartools)
- **Recent features (2026):**
  - v5.66.0 (19 May) added chat overlay widgets: "Chat" with display settings, and "Chat (Advanced)" with your own HTML/CSS. It also added Power-ups and Watch Streaks.
  - v5.66.1 added `$allowHtml` for Overlay Alerts, with a warning against untrusted data. v5.66.0 fixed a potential **XSS** in Overlay Alert.
  - v5.67.0-beta1 (24 Aug) added **Control Decks** (grid, pages, folders, phone control by QR code on the LAN), a rebuilt **plugin system with Plugin Manager**, **pinned chat messages**, and Rotating Text, Twitch Poll and Twitch Prediction widgets ([releases](https://github.com/crowbartools/Firebot/releases)).
  - v5.65 introduced the **Overlay Alert** effect (image or video, color-accented text and sound in one action) and **Overlay Widgets**: persistent Countdown, Progress Bar and Text ([v5.65.0-beta1](https://newreleases.io/project/github/crowbartools/Firebot/release/v5.65.0-beta1)).
- **Overlay server:** The internal web server sends no-cache headers, and overlay page titles include the instance name ([v5.60 beta](https://newreleases.io/project/github/crowbartools/Firebot/release/v5.60.0-beta2)).
- **Gaps:** It is Twitch-only in core, and there is no free-form WYSIWYG canvas. Widgets are configured with forms.
- **Borrow:**
  - "Effect lists" UX.
  - Persistent widgets that are separate from transient alerts.
  - Phone control deck by QR.
  - Plugin manager.
  - Pinned messages.
  - Pronoun variable.
  - Its security stance on HTML in alerts.

### Lumia Stream
- **Features:**
  - Overlay element types: Alerts, Text box, Goals, Image, Audio, Video box, Slideshow, Shapes, Labels, Browser sources, Chatbox, Shoutout/Clips ([forum](https://forum.lumiastream.com/t/what-is-lumia-stream-overlays/572)).
  - **HUD mode** shows the overlay on your own monitor.
  - Alert **variations** are Premium-only ([pricing](https://lumiastream.com/pricing)). Variation examples are "10th sub" and "100th gifted sub". Alert options include **Skip Queue**, "Skip Replay" for gift subs and choosing whether to alert per gift or per bomb, TTS, chatbot message, and advanced actions (OBS, Spotify, MIDI, custom code) ([forum](https://forum.lumiastream.com/t/alerts-what-you-can-setup-in-your-alerts/729)).
- **Custom overlay code API (good prior art):**
  - Tabs: HTML, CSS, JS, **Configs** (field schema) and **Data**.
  - `{{key}}` substitution in all three code tabs.
  - `Overlay.on('chat'|'alert'|...)`, `Overlay.callCommand`, `Overlay.setVariable/getVariable`, and `Overlay.saveStorage` (scoped by `codeId`).
  - Config field types: `input, textarea, number, slider, checkbox, dropdown, multiselect, colorpicker, fontpicker, imageupload, soundupload, videoupload, actionbutton`, with `visibleIf` rules ([dev docs](https://dev.lumiastream.com/docs/custom-overlays/custom-overlays-documentation)).
- **Gaps (from our point of view):** Variations are paywalled, and the product is closed source.
- **Borrow:**
  - The Configs schema with `visibleIf`.
  - Storage scoped per widget.
  - HUD mode.
  - Lights integration.
  - Per-alert "skip queue".

### Mix It Up
- **Overlay system:**
  - Local server at `http://localhost:8111/overlay`.
  - **Multiple endpoints**, each with its own URL and shared custom HTML/CSS/JS.
  - Positioning modes: **Simple (9-grid), Percentage, Pixels, Random**.
  - Entrance and exit animations, plus extra timed animations ([overlay docs](https://mixitup.bot/docs/overlays)).
  - 19 widget types: Chat, Custom, End Credits, Event List, Game Queue, Goal, HTML, Image, Label, Leaderboard, Persistent Emote Effect, Persistent Timer, Poll, Stream Boss, Text, Video, Wheel, YouTube. Any widget can use a **Single Widget URL** ([widgets](https://mixitup.bot/docs/overlays/overlay-widgets)).
- **Other:** A community library of ready-made commands you add with one click, automod, currency and giveaways ([site](https://mixitup.bot/)).
- **Gaps:** Windows only. No canvas designer.
- **Borrow:**
  - Endpoints and single-widget URLs.
  - "Random position in range".
  - End Credits and Stream Boss widgets.
  - Community command library.

### Social Stream Ninja (SSN), deep dive

**Ingestion model** ([standalone modes](https://socialstream.ninja/docs/standalone-app-modes.html), [event reference](https://socialstream.ninja/docs/event-reference.html)):

| Mode | How it works | Pros | Cons |
|---|---|---|---|
| **Standard (DOM capture)** | The extension or app opens the platform's (pop-out) chat page and reads the rendered DOM | Works for 120+ sites with no API keys, including TikTok, Instagram, Zoom and Teams | Breaks when layouts change. Mostly gives chat only. The page must stay open and not suspended |
| **WebSocket/API** | Platform APIs and sockets: Twitch EventSub, YouTube Data API bridge, Kick websocket bridge | Richer events (subs, gifts, raids, bans) and works in the background | Needs tokens and setup |
| **Polling/Legacy** | Periodic fetch | Fallback | Latency |
| TikTok presets | Auto / Standard / Local Signer / Polling / Proxy signing | Resilience | Fragile, and has ToS exposure |

**Event capability by mode** (from the event reference): Twitch subs, follows, raids, hype trains and bans need WebSocket mode. DOM capture gets bits, rewards and viewer count. YouTube's API bridge gives Super Chats, Stickers, Jewels, memberships, gifts and bans, but **does not give instant subscriber alerts** (only delayed polling). Kick DOM capture gives viewer count only, and alerts need the websocket.

**Normalized message schema (good prior art):**
- Fields: `type` (platform), `chatname`, `username`, `userid`, `chatmessage`, `chatimg`, `chatbadges[]`, `id`, `event`, `hasDonation` (display string), `donoValue` (USD number), `membership`, `subtitle`, `meta{}`, `contentimg`, `textonly`, `firsttime`, `lastactivity`.
- **Deletion messages:** `{delete:{type,id}}`, `{delete:{type,meta:{messageId}}}`, `{delete:{type,userid}}` ([event reference](https://socialstream.ninja/docs/event-reference.html)).

**Dock and overlay URL parameters** ([parameters.md](https://raw.githubusercontent.com/steveseguin/social_stream/main/parameters.md)) give an almost complete list of filters that streamers actually ask for:

| Area | Parameters |
|---|---|
| Commands / content | `hidecommands` (starts with `!`), `hideshortmessages`, `hidenumbers`, `noemojisonly`, `attachmentsonly`, `hidequestions`/`onlyquestions`, `badkarma` (sentiment), `normalize` (diacritics) |
| Users / bots | `hidefrom`/`exclude`, `onlyfrom`, `namefilter`, `botlist`/`botnames`, `hidebots`, `hidebotnames`, `hidehosts` |
| Roles | `showonlymods`, `showonlyvips` |
| Platforms | `onlytwitch`, `hidetwitch`, `hidesource`, `showsourcename` |
| Sanitizing | `striphtml`, `striplinks` (+`allowmodlinks` etc.), `stripemoji`, `stripnameemoji` |
| Emotes / badges | `staticemotes` (freezes animated 7TV/BTTV/FFZ/Twitch/Kick emotes), `emoji` size %, `hidebadges`, `limitbadges`, `showvipbadge` |
| Lifetime | `showtime` (auto-hide ms), **`delaytime`** (delay display, which gives mods time to delete), `limit`, `fadein`/`fadeout`, `fadedtop` |
| Highlights | `firsttimehighlightcolor`, `donationhighlightcolor`, `memberhighlightcolor`, `questionhighlightcolor` |
| TTS | `tts`, `voice`, `rate`, `pitch`, `ttscommand` (`!say`), `ttscommandmembersonly`, `readevents`; providers Google, ElevenLabs, Speechify, Gemini, OpenAI-compatible, **Kokoro, Kitten (local)** |

Not found: a dedicated word or **regex** filter parameter, and a pronouns option.

**Remote API:**
- `wss://io.socialstream.ninja` with numbered channels: 1 remote control, 2 dock output, 4 chat from the extension, 5 waitlist.
- HTTP `/{session}/{action}/{target}/{value}` and SSE.
- Actions: `sendChat`, `clearOverlay`, `nextInQueue`, `autoShow`, `blockUser`, `pin`/`unpin`, timers, giveaways and waitlist ([commands](https://socialstream.ninja/docs/commands.html)).
- **Note:** this relay goes through SSN's cloud server. Our owner wants **local-only** routing.

**Other features:** featured-message overlay (manual or auto), preview in the dock before going live, a **StreamElements widget converter**, the Event Flow visual editor, relaying chat between platforms, and AI (Ollama).

**Borrow:**
- The normalized schema.
- The full filter vocabulary, as **named profiles** rather than URL params.
- `delaytime`.
- Featured-message queue.
- The DOM-capture fallback for long-tail platforms (an optional plugin).
- SE widget import.

**License note:** SSN is GPL-3.0. Copying its code would force GPL on us, so reimplement it instead.

### Touch Portal
- Free with 2 pages. Pro is $13.99 one-time. Paid add-ons: multiple devices, icon editor.
- Button logic: toggle, radio, master-slave. Also **Values** (shared state) and **Events** (react to state).
- Hosts: Win, mac, Linux beta ([site](https://www.touch-portal.com/)).
- **Borrow:** a deck editor with state-driven button visuals. This is SAMMI deck parity.

### Aitum
- Rule-based automation (trigger, then a sequence of actions) with multi-PC support ([directory](https://contentcreators.com/tools/aitum)).
- Free **Aitum Vertical** OBS plugin with a separate vertical canvas.
- **Aitum Multistream**: Win/mac/Linux, OBS ≥30 ([OBS forum](https://obsproject.com/forum/resources/aitum-multistream.1991/)).
- A Stream Deck plugin toggles "Twitch Redemption Groups" ([Elgato](https://apps.elgato.com/plugins/tv.aitum.sdplugin)).
- **Borrow:**
  - Redemption groups that you enable or disable together.
  - Multi-PC agents.
  - Awareness of vertical canvases, so overlays also target a 1080×1920 canvas.

### Kruiz Control
- Runs entirely inside an OBS browser source. Triggers are written as text pseudo-code in `triggers.txt`.
- Files listed in `fileTriggers.txt` run **sequentially (queued)** unless `#noqueue` is set ([repo](https://github.com/Kruiser8/Kruiz-Control)).
- **Borrow:** the explicit queued versus parallel semantics per action group. Streamer.bot has a similar "queues" concept.

### Nightbot / Fossabot (brief)
- Both are free cloud bots. Fossabot is known for **more configurable spam filters**, audit logs and custom roles/RBAC ([gyre](https://gyre.pro/blog/best-chatbot-to-use-for-streaming)).
- **Borrow:**
  - Spam filter categories: caps, symbols, links, emote flood, repetition, blocked terms.
  - Moderator audit log.
  - Both bots belong in our built-in bot list.

### Chatterino / Chatterino7 (multichat UX)
- Splits (side-by-side chats), tabs, popped-out windows, highlights and ignores, a `/mentions` panel, **Streamer Mode** (hides user content while OBS runs), and link previews off by default ([site](https://chatterino.com/)).
- **Filter language** ([wiki](https://wiki.chatterino.com/Filters/)):
  - Variables: `author.badges`, `author.subbed`, `message.content`, `message.length`, `channel.name`, `flags.first_message`, `reward.title`, `bits.amount`.
  - Operators: `contains`, `startswith`, `endswith`, `match r"regex"`/`ri"..."`, `&& || !`.
  - Example: `!(message.content startswith "!points")`.
- Chatterino7 adds 7TV paints, personal emotes and animated avatars.
- **Borrow:**
  - An expression filter language that compiles to the same AST as our GUI rule builder.
  - The mentions panel.
  - Streamer mode.
  - Per-split filters.

### Restream Chat
- Unified feed of all destinations in browser, desktop or OBS dock. Replies go to all destinations.
- Overlay is added as an **embed URL**, with no OBS account link needed ([blog](https://restream.io/blog/restream-chat-everything-you-need-to-know/)).
- **Borrow:** a "reply to all / pick targets" composer. Streamer.bot Chat also lets you choose target chats.

### StreamElements
- **Overlay editor:** cloud-hosted. Pick a resolution, then add widgets with "+". The AlertBox widget plays queued events, and the Custom Widget has HTML/CSS/JS plus fields ([getting started](https://docs.streamelements.com/overlays/getting-started)). There is also an overlay gallery ([support](https://support.streamelements.com/hc/en-us/articles/10474664619666-Introducing-Overlays-Gallery-and-My-Overlays)).
- **Custom widget events** ([events](https://docs.streamelements.com/overlays/custom-widget-events)):
  - `onWidgetLoad`, `onEventReceived` and `onSessionUpdate`.
  - Listeners: `follower-latest`, `subscriber-latest`, `tip-latest`, `cheer-latest`, `raid-latest`, `message`, **`delete-message`** (msgId), **`delete-messages`** (userId), `event:skip`, `kvstore:update`, `bot:counter`, `widget-button`.
  - Subscriber flags `gifted`, `sender`, **`bulkGifted`**, **`isCommunityGift`** and **`playedAsCommunityGift`** model gift-bomb merging.
  - `SE_API.store` is a cross-widget persistent key-value store.
- **Variations:** several per alert type. Optional "Pick an alert randomly when more than one matches", otherwise the oldest match wins ([blog](https://blog.streamelements.com/weve-got-updates-random-alerts-and-count-commands-1bd6fb059621)). Variations can be conditioned on subscription tier, including Prime ([support](https://support.streamelements.com/hc/en-us/articles/16789217829778-Setting-Up-Twitch-Alerts-with-StreamElements-Overlays)). There is an "Emulate" test button.
- **Borrow:**
  - The custom-widget field and event model, so we can offer a compatibility shim.
  - Deletion events.
  - The gift-bomb flags.
  - Random pick among matching variations.

### Streamlabs
- **General settings:** Alert Delay, Alert Moderation (approve within N seconds, or Unlimited), **Alert Parries** (whether a new alert interrupts the current one), Profanity Filter plus custom bad words ([setup guide](https://streamlabs.com/content-hub/post/setting-up-your-streamlabs-alerts)).
- **Global "Fast Design" with per-alert overrides:** layout (text over, under or next to the image; preset or manual sliders), duration, **text delay**, alert and text in/out animations plus **text animation while onscreen**, font, TTS with spam protection, sound and volume, and custom HTML/CSS/JS. A "Test Variation" button tests a single variation.
- **Variation conditions** ([variations](https://streamlabs.com/content-hub/post/how-do-alert-box-variations-work)):

| Event | Conditions |
|---|---|
| Twitch Follow | Randomly |
| Twitch Sub | Randomly, Months ≥ N, Months = N, Is gift, Tier is X, Is Prime, **User gifted ≥ N subs at once** |
| Twitch Bits | Randomly, Bits ≥ N, Bits = N |
| Twitch Raid | Randomly, Raid size ≥ N |
| YouTube Subscriber | Randomly |
| YouTube Member | Randomly, Months ≥ N, Level ≥ N, Level name is X, Gifted ≥ N at once |
| YouTube Super Chat | Randomly, Amount ≥ N, Amount = N |
| Tips | Randomly, Amount ≥ N, Amount = N, **Largest this stream**, Is recurring |

- **Template variables:** `{name}`, `{months}`, `{gifter}`, `{amount}`, `{count}`. HTML is allowed in templates ([params](https://streamlabs.com/content-hub/post/message-template-parameters-for-alert-box-widget-on-streamlabs-twitch-and-youtube)).
- **Borrow:**
  - The full condition vocabulary, including "largest this stream".
  - Global defaults with per-alert overrides.
  - Text delay.
  - Moderation hold.
  - Profanity filter.
  - Interrupt policy.

### OWN3D / Overlay Expert
- **OWN3D Pro:** an Overlay Maker with about 12,000 designs and about 500 alert designs. Widgets are free (chatbox, goals, countdown, labels). Automatic setup creates scene collections. Stream Pass is quarterly, with AI credits ([pro](https://own3d.tv/en/pro)).
- **Overlay Expert:** online builder with 60+ animations. Panel contents swap, change and scroll, and panels themselves **peek and hide**. It runs as a **Twitch Extension**, so overlays render on the viewer side, which lets console streamers use it ([devpost](https://devpost.com/software/overlay-expert-axz1cs), [profile](https://contactout.io/company/Overlay-Expert-32609)).
- **Borrow:**
  - "Rotator/peek" panel behaviours: cycling content, timed hide/reveal.
  - A theme/template gallery with one-click install.
  - Export to scene collection.

### 1UP Vision (not on the list, but the closest modern prior art for the designer)
- Free widgets get 7 layer types. Paid multi-layer overlays get 60+ layer types ([gallery](https://docs.1up.vision/widgets/widget-gallery.md)).
- Custom widgets run in a **sandboxed iframe** with HTML, CSS, JS, Fields and Data tabs.
- The `window.Vision` API has `on/once/off`, `context.get`, and variables with `interpolate()` supporting `{key}`/`{{key}}`.
- An **SE compatibility layer** emulates `onWidgetLoad`, `onEventReceived`, `onSessionUpdate`, `SE_API.store/counters/sanitize/cheerFilter/resumeQueue/setField` ([custom widgets](https://docs.1up.vision/custom-widgets)).
- A React Extensions SDK lets third parties add new layer types.
- Mods can co-edit overlays live, with permissions and an action log ([AlternativeTo](https://alternativeto.net/software/1up-vision/about/)).
- **Borrow:**
  - A sandboxed iframe widget with an SE shim.
  - An SDK for third-party element types, which fits our API-first goal.
  - Collaborative editing with roles.

### Botrix, Sound Alerts (brief)
- **Botrix:** Kick-focused cloud bot with commands, automod, points and song requests. Alerts are a browser-source URL, and the bot must be added as a moderator ([eklipse](https://blog.eklipse.gg/tools/top-5-kick-stream-extension.html)). Add "BotRix" to the bot list.
- **Sound Alerts:** viewers spend Bits, Coins or Channel Points to trigger sounds, videos and TTS. Up to 10 channel-point alerts on the free tier ([support](https://support.soundalerts.com/article/channel-points)). Coins and Fullscreen Effects shipped in Aug 2026 ([changelog](https://feedback.soundalerts.com/changelog)).
- **Borrow:** a "viewer sound board" using channel points, with Automod-aware TTS. Sound Alerts' bot account also belongs in the bot list.

### veadotube / VTube Studio (brief)
- **veadotube mini:** local WebSocket "nodes" for push-to-talk and `stateEvents` (avatar states). Clients must send a name. The server address is **randomized each launch**, so integrations need discovery through the instance file ([websocket](https://veado.tube/help/docs/websocket), [API](https://veado.tube/docs/tech/api/)).
- **VTube Studio:** public WebSocket API. A per-plugin permission popup appears, and permissions can't be revoked through the API. Hotkeys and item hotkeys use the item *instance ID*. Events include ModelLoaded, ModelMoved and HotkeyTriggered. VTS also has built-in Twitch Hotkey Triggers ([wiki](https://github.com/DenchiSoft/VTubeStudio/wiki/Twitch-Hotkey-Triggers)).
- **Designer tie-in:**
  - Alert variations should be able to fire VTuber actions (hotkey, expression, item drop) synchronized with the alert timeline. For example, "on enter + 300 ms, trigger VTS hotkey 'wave'".
  - TTS playback should emit mouth-open values for lip-sync.

## A.3 Features Streamer.bot / SAMMI lack that competitors have

| Capability | Who has it | Streamer.bot | SAMMI |
|---|---|---|---|
| WYSIWYG overlay/alert designer | SE, SL, Lumia, Vision, OWN3D, Overlay Expert | No (chat overlay only, experimental) | No |
| Alert variations with conditions and random pick | SL, SE, Lumia (premium) | No | No |
| Alert queue with interrupt policy, moderation hold, profanity filter | SL, Lumia (skip queue) | Action queues only | No |
| Gift-bomb merge flags | SE (`bulkGifted`, `playedAsCommunityGift`), Lumia | Gift Bomb trigger exists ([docs](https://docs.streamer.bot/api/triggers/twitch/subscriptions/gift-bomb)), no rendering | No |
| Persistent widgets (goal, timer, poll, prediction, credits, wheel, leaderboard) | Firebot, MIU, Lumia | No | No |
| 120+ platform multichat (DOM capture) | SSN | Twitch, YouTube, Kick | No |
| Filter profiles (bots, commands, roles, regex) per output | SSN (URL params), Chatterino (filter language) | CSS-only styling, highlights | No |
| Overlay display delay so mods can delete first | SSN `delaytime`, SL moderation delay | No | No |
| Featured/pinned message to overlay | SSN, Firebot | No | No |
| Custom widget SDK with config schema and SE import | Lumia, Vision, SSN converter | No | No |
| Plugin manager / marketplace | Firebot 5.67, MIU community library, SE/OWN3D galleries | Extensions site (manual import) | Extensions (manual) |
| Phone control deck over the LAN by QR | Firebot, Touch Portal | No | Decks (desktop) + Android app **(verify)** |
| Visual flow editor | SSN Event Flow | List-based | Block-based |
| Local AI TTS voices | SSN (Kokoro, Kitten) | via Speaker.bot (cloud/OS voices) | No |
| Lights + audio-reactive "Fuze" | Lumia | via HTTP/plugins | via extensions |
| HUD mode (overlay on your own monitor) | Lumia | No | No |
| Collaborative mod editing with audit log | Vision, Fossabot | No | No |
| Multiple overlay endpoints / single-widget URLs | MIU, Firebot | n/a | n/a |

## A.4 Top 15 competitor features worth adopting (ranked)

| # | Feature | Source(s) | Why |
|---|---|---|---|
| 1 | **One designer for alerts, chat rows and widgets, with conditional variations** (tier, months, amount ≥/=, gift count, largest this stream, random weight) | SL, SE, Lumia, Vision | Core owner requirement. A unified template model removes duplicate UIs |
| 2 | **Alert queue engine**: priority lanes, interrupt/parallel policy, global delay, skip/replay/pause API, persistence | SL "parries"/delay, Lumia "skip queue", Kruiz queued files | The biggest gap in Streamer.bot and SAMMI |
| 3 | **Gift-bomb / combo merge** (one bomb alert, recipients suppressed or listed; follow-train coalescing) | SE flags, Lumia option, SB Gift Bomb trigger | Avoids 50 alerts for a 50-gift bomb |
| 4 | **Named filter profiles** (dock, overlay, TTS) with a built-in bot list, `!command` hide, words/regex, roles, platforms, links, emote-only | SSN, Chatterino | Each output needs different strictness |
| 5 | **Deleted-message and purge sync, plus optional overlay display delay** | SE `delete-message(s)`, SSN `delaytime` | Moderation safety on stream |
| 6 | **Moderation hold and profanity filter for alerts/TTS** (approve in the dock) | SL Alert Moderation | Hate-raid protection |
| 7 | **Featured/pinned message to overlay**, with a queue and auto-feature rules | SSN, Firebot 5.67 | Engagement favourite |
| 8 | **Custom widget SDK** (sandboxed iframe, fields schema with `visibleIf`, scoped storage, **SE widget import shim**) | Lumia, Vision, SE, SSN | API-first goal. Instantly reuses the large SE widget ecosystem |
| 9 | **Test-fire / emulate per variation**, plus a dock preview before going live | SE Emulate, SL Test Variation, SSN | Fast iteration |
| 10 | **Persistent widgets**: goals, counters/labels, countdown, polls/predictions, hype train, event list, rotating text, end credits, wheel | Firebot, MIU, Lumia | Replaces SE/SL cloud widgets with local ones |
| 11 | **Multiple overlay endpoints and single-widget URLs** with instance names | MIU, Firebot | Per-scene sources, separate audio control |
| 12 | **Hybrid ingestion**: official APIs first, optional DOM-capture plugin for long-tail platforms (TikTok, Rumble, etc.) | SSN | Breadth without blocking on APIs |
| 13 | **Phone/LAN control deck by QR**, with state-driven button visuals | Firebot, Touch Portal, SAMMI | SAMMI parity, better UX |
| 14 | **Plugin manager and community gallery** (one-click import of actions, overlays, alert packs) | Firebot, MIU, SE/OWN3D | Ecosystem growth |
| 15 | **Chat UX niceties**: pronouns, first-time highlight, `/mentions`, Streamer Mode, 7TV/BTTV/FFZ with live 7TV updates | Chatterino, Firebot, SSN | Expected by Twitch-native users |

Honourable mentions: Lumia HUD mode, lights/Fuze, Event Flow visual editor (SSN), local AI TTS voices, redemption groups (Aitum), mod co-editing with audit log (Vision), "largest donation this stream" condition (SL).

---

# Part B: WYSIWYG Overlay & Alert Designer

## B.1 Prior-art comparison

| Aspect | StreamElements | Streamlabs | Lumia | Mix It Up | Firebot | 1UP Vision | Overlay Expert |
|---|---|---|---|---|---|---|---|
| Canvas | Fixed resolution chosen at creation, free positioning | Per widget; alert layout presets or manual sliders | Overlay canvas with layers | 9-grid, %, px or random positioning | Forms; widget position settings | Multi-layer canvas, 60+ layer types | Panel-based |
| Alerts | AlertBox widget, a tab per event | Alert Box, global defaults with per-alert overrides | Alert layer with variations (premium) | Alert via overlay actions | Overlay Alert effect | Keyframe-animated alert layers | Alert layers |
| Conditions | Tier/amount/etc., random among matches | Rich list (Streamlabs table in A.2) | "Nth sub" etc. | Via command logic | Via filters/conditions | **(verify)** | **(verify)** |
| Animations | Presets (animate.css-style) **(verify)** | In/out plus text animation | Presets, virtual-light effects | Entrance/exit plus timed | motion-v (Vue) | Keyframes | 60+ presets, peek/hide |
| Custom code | HTML/CSS/JS + fields JSON | HTML/CSS/JS toggle | HTML/CSS/JS + Configs + Data | HTML and Custom widgets | Chat (Advanced) HTML/CSS | Sandboxed iframe + SE shim + React SDK | n/a |
| Hosting | Cloud | Cloud | Local app server | `localhost:8111` | Local server | Cloud | Cloud / Twitch Extension |

## B.2 Design principles

1. **Same renderer everywhere.** The editor canvas, the dock preview and the OBS browser source all mount one `@streamerlive/renderer` package. WYSIWYG holds by construction.
2. **Templates + variations + bindings + animations** is the single abstraction for alerts, chat rows, event-list rows, labels and widgets.
3. **Local-first.** Overlays are served by the app on `127.0.0.1`. No cloud relay. Optional authenticated LAN mode for dual-PC setups.
4. **Declarative JSON documents** that are diff-friendly, versioned, migratable and shareable as packages.
5. **Safe by default.** All user and platform text is escaped. Custom code runs only in sandboxed iframes (Firebot shipped an XSS fix in its alert effect, which shows the risk is real ([releases](https://github.com/crowbartools/Firebot/releases))).
6. **API-first.** Every editor action has an equivalent in the HTTP/WS API. Third parties can register element types and condition variables through the plugin SDK.

## B.3 Architecture

```
┌──────────────────────── Streamer.live core (local) ──────────────────────────┐
│ Platform connectors ──► Event normalizer ──► Event bus ──┬─► Action engine    │
│ (Twitch EventSub, YouTube streamList,                     ├─► Alert Router ──► Queue lanes
│  Kick webhooks/ws, plugins, DOM-capture)                  ├─► Chat pipeline ─► Filter profiles
│                                                           └─► State store (goals, counters, session)
│ Overlay server: HTTP (static runtime, assets by hash) + WebSocket (/ws/overlay/:id?token=)        │
└───────────────────────────────────────────────────────────────────────────────┘
        ▲ editor (app window)           ▲ OBS dock (chat, queue, moderation)   ▲ OBS browser sources
        │ same renderer, edit mode      │ same renderer, dock mode              │ renderer, runtime mode
```

- The **Alert Router** matches events against alert rules and variations, then pushes `AlertJob`s into **queue lanes**.
- The runtime **acks** play start and play end. The queue advances on ack, or on a timeout fallback.
- The **chat pipeline** sends messages through *filter profiles*. Each overlay, dock or TTS output subscribes to a profile.

## B.4 Canvas model

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Pure absolute (x, y, w, h at a design resolution) | Matches OBS mental model and every competitor. Simple to drag | Not responsive. Lists need manual layout | **Top level** |
| Pure flex/flow (page builder) | Responsive, good for lists | Fights broadcast-style free placement | No |
| **Hybrid**: absolute top level, plus **constraints/anchors** (Figma-style) and **Stack** (flex) containers | Responsive to canvas size. Lists and alert text auto-layout | Slightly more UI | **Recommended** |

- Each document has a `canvas: {width, height}` design resolution: 1920×1080 by default, with presets for 2560×1440, 3840×2160, **1080×1920 vertical** (Aitum Vertical) and custom sizes. The default can be read from OBS via obs-websocket `GetVideoSettings`.
- **Scale modes** at runtime: `fit` (uniform scale, letterboxed), `fill`, `stretch`, `none` (1:1). The runtime reads `window.innerWidth/innerHeight` and applies one CSS `transform: scale()` to the root. It can be overridden with URL params `?scale=fit&w=1920&h=1080`.
- **Constraints per element:** horizontal `left|right|center|left+right|scale`, vertical `top|bottom|center|top+bottom|scale`. This lets one design serve both 16:9 and 9:16, together with per-canvas overrides ("variants by aspect ratio").
- **Editor aids:**
  - Rulers and guides, snap to edges and centers, a safe-area overlay, a grid.
  - The OBS scene screenshot as background, through obs-websocket `GetSourceScreenshot`.
  - Lock, hide, group, align and distribute, z-order.
  - Nudge with arrow keys.

## B.5 Layers & element types

The layers panel is a tree with lock/hide/solo per layer, plus groups and masks. Every element has `frame`, `constraints`, `style`, `bindings`, `visibleIf` (rule), `animations` and `events` (on-click for dock or interactive use).

| Element | Purpose | Key properties / bindings | Notes |
|---|---|---|---|
| **Text** | Labels, alert lines | Rich segments, `{{var}}` tokens, per-token color (Firebot "color-accented fields"), font (local/Google), stroke, shadow, gradient fill, auto-fit/shrink, max lines, per-letter/word animation | Emotes inside text render as inline `<img>` |
| **Image** | Static or animated art | src (asset hash or URL binding, e.g. `{{user.avatarUrl}}`), fit, radius, filters | PNG, WebP, AVIF, GIF, APNG, SVG |
| **Video** | Alert videos, stingers | src, loop, muted, volume, playbackRate, chroma-free alpha | Prefer **WebM VP9 with alpha**. Verify H.264/HEVC support in the OBS CEF build **(verify)** |
| **Lottie** | Vector animations | `.lottie`/JSON, segment play, speed, color slots | `@lottiefiles/dotlottie-web` (MIT) |
| **Rive** (optional) | Interactive state-machine animations | state machine inputs bound to variables | `@rive-app/canvas` (MIT) |
| **Audio** | Alert sounds, music beds | src, volume, delay, duck others | Play through the browser source ("Control audio via OBS") or app-native output. See open questions |
| **TTS** | Spoken message | voice (Speaker.bot-equivalent engine), text template, filters, max length | Generated by core, delivered as an audio blob. Emits viseme/mouth events for VTuber lip-sync |
| **Shape** | Rect, ellipse, line, polygon, SVG path | fill, gradient, border, blur, backdrop-filter | CSS/SVG |
| **Progress / Goal** | Sub, bit or donation goals | value/target bindings, linear/radial/segmented, milestone markers, overflow behaviour | Also covers **Hype Train meter** (level, progress, timer) |
| **Label / Counter** | "Latest follower", "Top donor", session counts | binding + format (number, currency, duration, relative time) | Stream-labels parity |
| **Event List** | Recent events ticker | row template (Stack), max rows, direction, filter | MIU/SE parity |
| **Chat List** | Multichat overlay | filter profile, row template with variations, max messages, lifetime, direction, emote sizing | See B.12 |
| **Avatar / Badges / Platform icon / Pronouns** | Building blocks for rows | bound to message/user context | Used inside row templates |
| **Timer / Countdown / Clock** | Subathon, BRB | source (state store), format, end action | Firebot parity |
| **Rotator / Slideshow / Rotating text** | Cycle content, peek/hide | items, interval, transition | Overlay Expert, Firebot, Lumia |
| **Poll / Prediction** | Live Twitch poll/prediction | bound to platform state | Firebot 5.67 parity |
| **Emote wall / rain / particles** | Effects | emote source, physics | **Canvas (PixiJS, MIT)** inside a DOM element |
| **Stack (auto-layout)** | Flex container | direction, gap, align, wrap, padding | Used for chat rows, alert text blocks |
| **Mask / Clip group** | Reveal effects | shape mask, `clip-path` | |
| **Alert Slot** | Where alerts play | lane/queue id, region, z-order | The "AlertBox" equivalent on an overlay |
| **Custom Widget** | HTML/CSS/JS | fields schema, data, events, storage | Sandboxed iframe, see B.10 |
| **Browser embed** | External URL | url, refresh interval | iframe |
| **Plugin element** | 3rd-party element types | registered through the SDK | Vision-style extension SDK |

## B.6 Animation system

**Model.** Each element has:
- `enter`, `idle` (loop) and `exit` **tracks**.
- An optional **timeline** of keyframes, measured relative to the alert or template start, so text can arrive 500 ms after the image ("text delay", as in Streamlabs).
- **Timed actions** on the same timeline: play sound, trigger a VTS hotkey, fire an app action, emit a custom event.

**Presets.** A JSON library of keyframe presets, animate.css-style: fade, slide, zoom, bounce, flip, back, light-speed, plus text effects (wave, wiggle, typewriter, per-letter stagger). Presets are data, not code, so users can edit and share them.

**Engine choice:**

| Option | License | Verdict |
|---|---|---|
| **Web Animations API (WAAPI)** | Native | **Primary runtime engine.** Uses the compositor (transform/opacity), and can be paused and scrubbed via `currentTime`, which drives the editor timeline |
| CSS `@keyframes` | Native | Generated from the same preset JSON for idle loops and custom-CSS users |
| **Motion** (`motion` 14.3.0, Oct 2026) | MIT ([npm](https://www.npmjs.com/package/motion)) | **Use** for springs, sequences and stagger helpers. Vanilla `animate()` works from a script tag ([docs](https://motion.dev/docs/quick-start)) |
| anime.js 4.5.0 | MIT | Acceptable alternative |
| **GSAP** 3.15.0 | "Standard no-charge" license. Free, including commercial use, **but it prohibits use in "tools that allow users to build visual animations without code" that compete with Webflow's visual animation builder** ([license](https://gsap.com/community/standard-license/), [Webflow](https://webflow.com/blog/gsap-becomes-free)) | **Avoid** in the designer and runtime. Our product is literally a no-code visual animation builder. Users may still load GSAP inside their own custom widgets |
| Theatre.js | core Apache-2.0, **studio AGPL-3.0**, last release May 2024 | Avoid embedding the studio |

- **Timeline editor UI:** `@xzdarcy/react-timeline-editor` (MIT) if React, otherwise build in-house (tracks and keyframes are simple).
- **Performance in OBS CEF:**
  - Animate only `transform`, `opacity` and `filter`. Set `will-change` on active alert nodes.
  - **Preload before play**: images decoded, `document.fonts.load()`, audio buffered, video `canplaythrough`, Lottie loaded. Then send `ready`, and only then start the timeline.
  - Release DOM nodes after exit. Cap chat nodes.

## B.7 Template variables & expressions

- **Syntax:** `{{path | filter:arg}}`. This is compatible with Lumia (`{{key}}`) and Vision (`{key}`/`{{key}}`) ([Lumia](https://dev.lumiastream.com/docs/custom-overlays/custom-overlays-documentation), [Vision](https://docs.1up.vision/custom-widgets)). Importers translate Streamlabs `{name}` and Streamer.bot `%var%` **(owner decision)**.
- **Context namespaces:**
  - `event.*` (normalized event)
  - `user.*` (displayName, login, avatarUrl, color, badges[], pronouns, roles, isFirstTime, platform)
  - `amount.*` (value, currency, formatted, usd)
  - `sub.*` (tier 1/2/3/prime, months, streak, isGift, gifter, giftCount, communityGiftTotal)
  - `stream.*` (session counters, largest tip this stream)
  - `global.*` (app variables), `fields.*` (widget fields), `time.*`
- **Filters:** `upper`, `lower`, `truncate:n`, `currency`, `number`, `plural:"sub":"subs"`, `default:"x"`, `ordinal`, `duration`, `date`.
- **No `eval`.** Templates compile to a tiny AST. HTML is escaped unless the binding is marked `rich` (emote/mention segments), which renders through a safe node builder.

## B.8 Alert model: rules, variations, conditions

```
AlertSet (attached to an Alert Slot / lane)
 └─ Rule per event kind (twitch.sub, twitch.gift_bomb, twitch.cheer, twitch.follow, twitch.raid,
    twitch.hype_train.*, youtube.member, youtube.member_milestone, youtube.gift_membership,
    youtube.superchat, youtube.supersticker, youtube.subscriber(polled), kick.sub, kick.gift,
    kick.kicks, tip.*, custom.*)
     ├─ default variation (inherits AlertSet globals: layout, duration, font, TTS, sounds)
     └─ variations[] { when: Rule AST, priority, weight, enabled, overrides… }
```

- **Matching:**
  1. Evaluate variations in **priority order**.
  2. Collect every match within the top priority band.
  3. Pick by **weight** (SE's "random when several match"), or take the first.
  4. If nothing matches, use the default.
- **Rule AST** is JSON with `all/any/not`, comparisons (`== != > >= < <=`), `in`, `contains`, `startsWith`, `matches` (regex), `between`, plus `random(p)` and `isLargestThisStream(path)`.
  - The same AST powers **chat filter and highlight rules**, so there is one engine and one UI.
  - A text form, Chatterino-like (`sub.tier == 3 && !sub.isGift`), compiles to the same AST for power users.
- **Condition presets** in the UI mirror the Streamlabs list (A.2) plus Twitch Prime, YouTube level name, and Kick KICKs amount.
- **Inheritance:** variations store only *overrides*, as a JSON patch over the default. The UI shows inherited values greyed out with a "reset to default" control, following Streamlabs' Global "Fast Design" with per-alert overrides.
- **Template per variation:** a variation points to a **composition**: a mini-canvas with its own layers and timeline, sized to the Alert Slot. Simple variations reuse the default composition and only change text, media or sound.

## B.9 Alert queue, priority, combo/merge

| Feature | Design |
|---|---|
| **Lanes** | Named queues (e.g. `main`, `minor`, `tts`). Each Alert Slot binds to a lane. Lanes run independently, so a follow ticker can play while a sub alert plays |
| **Priority** | Numeric per rule/variation (defaults: hype train > raid > gift bomb > sub > bits/superchat > member > follow). Optional preemption: a higher priority can *interrupt* the current alert (Streamlabs "parries") or wait |
| **Concurrency** | `maxConcurrent` per lane (default 1). Parallel mode for lanes such as sound-only |
| **Global delay** | Delay before showing (SL "Alert Delay") to sync with stream latency |
| **Moderation hold** | Optional per rule. Jobs wait in the dock for **approve/deny** with a timeout (auto-approve or auto-deny), or unlimited (SL "Alert Moderation") |
| **Profanity / filter** | Run user-supplied text (names, messages, TTS) through the filter profile. Mask or drop |
| **Combo / merge** | `mergeWindowMs` + `mergeKey`. **Twitch:** a `community_sub_gift` chat notification is followed by N `sub_gift` notices sharing a community-gift id. Play one *gift-bomb* alert (`{{sub.giftCount}}`) and mark the children `playedAsCommunityGift` (SE semantics) so they are suppressed, collapsed into a recipient list, or shown as a mini-ticker. **YouTube:** `membershipGiftingEvent`, then N `giftMembershipReceivedEvent` ([API](https://developers.google.com/youtube/v3/live/docs/liveChatMessages)), linked by the associated gifting-message id **(verify field)**. **Kick:** `channel.subscription.gifts` is already aggregated ([Kick](https://docs.kick.com/events/event-types)). **Follow/raid trains:** coalesce N follows within X seconds into "N new followers" |
| **Dedup** | Drop duplicate events by platform id. Handle Twitch shared-chat duplicates (`shared_chat_*` notice types) |
| **Overflow** | Max lane length. Overflow policy: drop lowest priority, or summarise ("+12 more follows") |
| **Controls** | API and dock: pause/resume lane, skip current, replay last N, clear, test-fire, re-queue. SE has the equivalent `event:skip` |
| **Persistence** | Queue survives app restart. Replay history kept for the session |
| **Ack protocol** | Runtime sends `alert.started` and `alert.finished`. The queue advances on `finished`, or on `duration + grace` if the source is hidden. Avoid OBS "shutdown source when not visible" stalls: if no runtime is connected to a lane, either hold or auto-skip (configurable) |

## B.10 Custom widgets (HTML/CSS/JS)

- **Isolation:** `<iframe sandbox="allow-scripts">` with no `allow-same-origin`. Communication is `postMessage` only, through a tiny bridge, under a strict CSP. Network access is opt-in per widget.
- **API shape** (superset of Lumia and Vision):
  - `Widget.on(event, fn)`, `Widget.fields`, `Widget.vars.get/subscribe`
  - `Widget.storage.get/set` (scoped)
  - `Widget.callAction(name, args)`, `Widget.chat.send()`
  - `Widget.ready()` and `Widget.done(alertId)` for queue acks
- **Fields schema** (union of the SE, Lumia and Vision field types): `text, textarea, number, slider, checkbox, dropdown, multiselect, colorpicker, fontpicker, image, sound, video, button, hidden`, with `group`, `order`, `visibleIf`.
- **SE compatibility shim:** emulate `onWidgetLoad`, `onEventReceived` (listener names such as `subscriber-latest`, `message`, `delete-message`), `onSessionUpdate`, and `SE_API.store/counters/resumeQueue/setField/cheerFilter`. Vision and SSN both found this worth doing ([Vision](https://docs.1up.vision/custom-widgets), [SSN](https://github.com/steveseguin/social_stream)).

## B.11 Preview & test-fire

- **Editor preview** is the real renderer in edit mode. Selection overlays sit in a separate layer, so the content DOM matches the runtime.
- **Mock event generator:**
  - Presets per event type and per variation ("Fire Tier 3", "Fire 50-gift bomb", "Fire $100 Super Chat").
  - Randomized names, avatars and messages, including long names, RTL text, emoji-only messages and 7TV zero-width emotes.
  - "Fire to OBS" (sends to the live lanes, flagged as a test) versus "Preview only".
- **Timeline scrubbing** sets WAAPI `currentTime`. Play, loop and step frames.
- **Canvas preview modes:** 16:9 and 9:16, with an OBS scene screenshot as backdrop and safe areas.
- **Dock preview** before going live (SSN does this): see queued alerts and featured messages before they render.

## B.12 Multichat overlay / dock requirements

**Pipeline:** Ingest → Normalize → Enrich (emotes, badges, pronouns, roles, first-time, shared-chat source) → Moderation state (held / deleted / purged) → **Filter profile** (per output) → **Highlight rules** → Render (dock / overlay / TTS / featured).

| ID | Requirement | Details / platform mapping | Prior art |
|---|---|---|---|
| MC-01 | **Named filter profiles** | e.g. *Dock (everything, bots dimmed)*, *Overlay (strict)*, *TTS (strictest)*. Every output picks a profile. Profiles are editable in the GUI or as text expressions | SSN params, Chatterino filters |
| MC-02 | **Word / phrase / regex exclusion** | Case-insensitive, whole-word option, Unicode normalization (`normalize` diacritics, leetspeak and homoglyph folding). Regex with a timeout guard (RE2-style or a length cap) | SSN `normalize`, Chatterino `match ri"..."` |
| MC-03 | **Known-bot exclusion** | Built-in, updatable list per platform (below), plus the streamer's own bot accounts, plus user additions. Modes: hide, hide name only, dim | SSN `hidebots`/`botlist` |
| MC-04 | **Command exclusion** | Hide messages starting with configurable prefixes (`!` by default; also `?`, `$`). Optionally also hide bot replies to commands | SSN `hidecommands` |
| MC-05 | **Per-platform toggles** | Show/hide per platform and per channel (multi-channel and shared chat) | SSN `onlytwitch`/`hidetwitch` |
| MC-06 | **Role filters** | broadcaster, mod, VIP, sub/member (with tier/level), founder, artist, verified, follower-age **(verify availability)**, new chatter. "Only show" or "hide" | SSN `showonlymods`, Chatterino `author.badges contains` |
| MC-07 | **Content filters** | Links (allow for roles), emote-only, numbers-only, short messages, all-caps, repeated characters, max length (truncate) | SSN, Fossabot |
| MC-08 | **Highlight rules** | Same Rule AST. Actions: row variation (style), sound, pin, feature, send to `/mentions`. Built-ins: mention of streamer, first-time chatter, donation/bits, member, question, mod, VIP, keyword | SSN highlight colors, Chatterino |
| MC-09 | **Message lifetime / fade** | `maxMessages`, `lifetimeMs` (auto-hide), exit animation, fade top edge, pause on hover (dock) | SSN `showtime`, `limit`, `fadedtop` |
| MC-10 | **Display delay** | Optional N-second overlay delay so mods can delete first. The dock is never delayed | SSN `delaytime` |
| MC-11 | **Deleted-message sync** | **Twitch:** EventSub `channel.chat.message_delete` (single), `channel.chat.clear_user_messages` (ban/timeout purge), `channel.chat.clear` (/clear). Hold AutoMod-caught messages via `channel.chat.user_message_hold`/`_update` ([EventSub](https://dev.twitch.tv/docs/eventsub/eventsub-subscription-types/)). **YouTube:** deletion arrives as a `tombstone`-type item, and `userBannedEvent` purges the user ([liveChatMessages](https://developers.google.com/youtube/v3/live/docs/liveChatMessages)). **Kick:** the official webhook list has `moderation.banned` but **no message-deleted event** ([Kick](https://docs.kick.com/events/event-types)), so use the unofficial chat websocket **(verify)** or purge on ban only. **Our dock moderation** deletes locally right away (optimistic). Deleted rows run an exit animation or a "message deleted" placeholder (configurable) | SE `delete-message(s)`, SSN delete schema |
| MC-12 | **Emotes** | Twitch native (EventSub message fragments), **7TV** (REST, plus live `emote_set.update` via `wss://events.7tv.io/v3`, [EventAPI](https://github.com/SevenTV/EventAPI)), **BTTV**, **FFZ** (global and channel), zero-width/overlay emotes stacked, cheermotes, Kick emotes (`[emote:id:name]` tokens **(verify)**), YouTube emoji and member emoji (the Data API gives text shortcodes; images need a lookup **(verify)**), Unicode emoji via Twemoji (`@twemoji/api`, MIT + CC-BY-4.0 graphics) or system font. Options: emote scale, larger emote-only messages, freeze animated emotes (SSN `staticemotes`), per-provider toggles | Chatterino, SSN |
| MC-13 | **Pronouns** | Alejo "Twitch Chat Pronouns" service ([pr.alejo.io](https://pr.alejo.io/)), cached per user and lazy-loaded. Toggle on/off. Firebot exposes `$pronouns` ([releases](https://github.com/crowbartools/Firebot/releases)) | Firebot |
| MC-14 | **Badges** | Twitch global and channel badge sets (Helix), YouTube owner/mod/member/verified flags, Kick identity badges. Limit count, hide set (SSN `limitbadges`, `hidebadges`) | SSN |
| MC-15 | **First-time chatter** | Use the platform flag where available (Twitch first-message). Fall back to a **local seen-users DB** per platform, which works everywhere (SSN `firsttime/lastactivity`). Also "first message this stream" and "returning after N days" | SSN, Chatterino `flags.first_message` |
| MC-16 | **Shared chat (Twitch)** | Show source-channel badge. Filter "only my channel". Dedup | EventSub `channel.shared_chat.*` |
| MC-17 | **Name colors & paints** | Platform color, readability correction against the background, 7TV paints (optional) | Chatterino7 |
| MC-18 | **Featured / pinned** | Click in the dock to feature a message on the overlay. Queue, auto-feature rules, timeout. Twitch pinned-message sync when the API allows it | SSN, Firebot 5.67 |
| MC-19 | **Dock UX** | Combined or per-platform tabs, reply-to with platform targeting, quick mod actions (delete, timeout, ban, per platform), user card (history, notes, watch-time), `/mentions`, Streamer Mode, search, pause-scroll | Chatterino, Restream, SB Chat |
| MC-20 | **TTS profile** | Separate filter profile. `!say`-style command gating, members-only option, read events, skip links, max length, per-user voice | SSN TTS params, Speaker.bot parity |

**Built-in bot list (seed; per-platform; updatable from a signed remote list; user-editable):**
`nightbot, streamelements, streamlabs, moobot, fossabot, sery_bot, wizebot, botrixoficial / botrix (Kick) (verify exact login), kickbot (verify), streamlootsbot (verify), soundalerts, blerp (verify), pokemoncommunitygame, own3d (verify), tangiabot (verify), kofistreambot, frostytoolsdotcom (verify), streamcaptainbot (verify), deepbot (verify), phantombot (verify), coebot (verify), botisimo (verify), songlistbot (verify), pretzelrocks (verify), commanderroot (verify), lumiastream (verify), mixitupbot (verify)` plus the streamer's own configured bot accounts. Store it as data with `{platform, login, kind: "chatbot"|"viewbot"|"alertbot"}`. Twitch's "chat bot" badge for bot-account senders may allow auto-detection **(verify)**.

## B.13 Serialization (JSON schema)

- **Authoring:** use **Zod 4** (MIT, 4.6.7) types as the source of truth. Export JSON Schema for third parties and validators (Ajv 8, MIT) and generate TypeScript types. `schemaVersion` changes go through migration functions.
- **Packaging:** `.slpack` = a zip containing `manifest.json`, `overlay.json`, `assets/<sha256>.<ext>` and `fonts/`. Assets are content-addressed and deduplicated across documents. Packs import with one click from the gallery or a file.

```jsonc
{
  "$schema": "https://schemas.streamer.live/overlay/1.json",
  "schemaVersion": 1,
  "id": "ovl_01JABCD...",
  "kind": "overlay",                     // overlay | alertset | chatTemplate | widget | composition
  "name": "Main – Alerts + Chat",
  "canvas": { "width": 1920, "height": 1080, "scaleMode": "fit", "background": "transparent",
              "variants": [{ "id": "vertical", "width": 1080, "height": 1920 }] },
  "assets": { "sha256-9f2c…": { "type": "image", "mime": "image/webp", "w": 512, "h": 512, "name": "tier3.webp" } },
  "fonts": [{ "family": "Inter", "source": "local", "weights": [400, 800] }],
  "layers": [
    { "id": "slot_main", "type": "alertSlot", "name": "Alerts",
      "frame": { "x": 660, "y": 80, "w": 600, "h": 400, "rotation": 0 },
      "constraints": { "h": "center", "v": "top" },
      "lane": "main", "alertSet": "as_main" },
    { "id": "chat", "type": "chatList", "name": "Chat",
      "frame": { "x": 40, "y": 520, "w": 460, "h": 520 },
      "constraints": { "h": "left", "v": "bottom" },
      "filterProfile": "overlay-strict", "maxMessages": 20, "lifetimeMs": 45000,
      "direction": "bottom-up", "rowTemplate": "tpl_chat_row" }
  ],
  "alertSets": {
    "as_main": {
      "defaults": { "durationMs": 7000, "textDelayMs": 400, "composition": "cmp_default",
                    "sound": { "asset": "sha256-aa…", "volume": 0.8 }, "tts": { "enabled": false } },
      "rules": [
        { "event": "twitch.sub", "priority": 40,
          "variations": [
            { "id": "t3", "name": "Tier 3", "priority": 30,
              "when": { "all": [ { "var": "sub.tier", "op": "==", "value": 3 },
                                 { "var": "sub.isGift", "op": "==", "value": false } ] },
              "overrides": { "composition": "cmp_tier3", "durationMs": 10000 } },
            { "id": "gift", "name": "Gifted", "priority": 20,
              "when": { "var": "sub.isGift", "op": "==", "value": true },
              "overrides": { "text": "{{sub.gifter}} gifted {{user.displayName}} a sub!" } }
          ] },
        { "event": "twitch.gift_bomb", "priority": 60,
          "merge": { "key": "event.communityGiftId", "windowMs": 4000, "suppressChildren": true },
          "variations": [
            { "id": "big", "when": { "var": "sub.giftCount", "op": ">=", "value": 50 }, "weight": 1,
              "overrides": { "composition": "cmp_bomb_big" } } ] },
        { "event": "youtube.superchat", "priority": 45,
          "variations": [
            { "id": "100plus", "when": { "var": "amount.usd", "op": ">=", "value": 100 } },
            { "id": "largest", "when": { "fn": "isLargestThisStream", "args": ["amount.usd"] } } ] }
      ] } },
  "compositions": {
    "cmp_tier3": { "width": 600, "height": 400, "layers": [
      { "id": "img", "type": "image", "src": { "asset": "sha256-9f2c…" }, "frame": { "x": 172, "y": 0, "w": 256, "h": 256 },
        "animations": { "enter": { "preset": "zoomInBounce", "durationMs": 600 },
                        "idle":  { "preset": "float", "durationMs": 2000, "loop": true },
                        "exit":  { "preset": "fadeOutUp", "durationMs": 400 } } },
      { "id": "title", "type": "text", "frame": { "x": 0, "y": 270, "w": 600, "h": 60 },
        "text": "{{user.displayName}} subscribed at Tier 3!",
        "style": { "font": "Inter", "weight": 800, "size": 44, "color": "#fff", "stroke": { "w": 4, "color": "#000" }, "align": "center" },
        "animations": { "enter": { "preset": "letterWave", "delayMs": 400, "staggerMs": 25 } } }
    ],
    "timeline": [ { "atMs": 300, "action": { "type": "vts.hotkey", "hotkey": "Cheer" } } ] }
  },
  "filterProfiles": { "overlay-strict": { "ref": "global:overlay-strict" } }
}
```

## B.14 Runtime (OBS browser source over local WebSocket)

- **URL:** `http://127.0.0.1:<port>/o/<overlayId>?token=<per-overlay token>[&variant=vertical&lane=main&scale=fit]`. Single-widget URLs: `/o/<overlayId>/w/<layerId>` (MIU pattern).
- **Boot:**
  1. Fetch the runtime bundle (no-cache index, hashed immutable assets; Firebot learned this ([v5.60](https://newreleases.io/project/github/crowbartools/Firebot/release/v5.60.0-beta2))).
  2. Open the WS and send `hello{overlayId, instanceName, capabilities, viewport}`.
  3. Receive `overlay.load{doc}`, then `state.snapshot`.
- **Server → runtime messages:**
  - `overlay.patch` (live edit hot-reload as JSON Patch)
  - `alert.prepare{jobId, variation, ctx}` (the runtime preloads, then replies `alert.ready`), followed by `alert.play{jobId}`
  - `alert.skip`, `lane.pause/resume`
  - `chat.message`, `chat.delete{id}`, `chat.purgeUser{platform,userId}`, `chat.clear{platform,channel}`
  - `emotes.update`, `state.patch` (goals, counters), `widget.event`, `test.fire`
- **Runtime → server messages:** `alert.ready/started/finished{jobId}`, `error{…}`, `visibility{hidden}` (Page Visibility API), `widget.action`, `perf{fps, droppedFrames}`.
- **Resilience:**
  - Reconnect with backoff, then resync with `state.snapshot`.
  - The queue holds when no runtime is subscribed to a lane, optionally auto-skipping.
  - The OBS "Refresh browser when scene becomes active" setting is supported.
- **Audio:** the browser source audio route requires the user to enable "Control audio via OBS". Document it, and detect it via obs-websocket if possible **(verify)**.
- **Security:**
  - Bind to localhost by default. LAN mode needs a token and an allowlist.
  - Every chat or user string is rendered through text nodes.
  - Custom widgets run in sandboxed iframes, and `allowHtml` is never used for chat-sourced data (Firebot's warning).

## B.15 DOM/CSS versus canvas rendering

| Criterion | DOM/CSS | Canvas (Konva/Fabric/Pixi) |
|---|---|---|
| Text quality, fonts, RTL, emoji, line-breaking | Native, best | Manual. Weaker shaping and wrapping |
| Emotes / GIF / WebP animated in text | `<img>` inline, trivial | Must decode frames manually |
| Custom HTML/CSS/JS widgets (SE ecosystem) | Native (iframe) | Impossible inside the canvas |
| CSS/WAAPI animations, filters, blend modes | Native, compositor-accelerated | Reimplemented per library |
| Editor hit-testing and handles | Overlay libraries (Moveable) | Built-in transformer |
| Many particles (emote rain, confetti) | Slow above about 500 nodes | **Best** |
| WYSIWYG parity with OBS CEF | Same engine (Chromium) | Same, but needs two models if mixing |

**Recommendation:** DOM/CSS for everything, with a **Canvas element type** (PixiJS 8, MIT) for particles and emote-wall effects only.

## B.16 Library recommendations (npm registry, 2026-10-18)

| Library | Version (date) | License | Role | Verdict |
|---|---|---|---|---|
| **moveable** / react-moveable / vue3-moveable / svelte-moveable | 0.53.0 / 0.56.0 (**2023-12-03**) | MIT | Drag, resize, rotate, snap guidelines, groups, warp, clip on DOM elements ([repo](https://github.com/daybrush/moveable): 10.8k★, 450 open issues) | **Use for v1, pinned and vendored.** Feature-complete but **unmaintained for about 3 years**. Budget a fork |
| **selecto** | 1.26.3 (2023-12) | MIT | Marquee/drag selection (pairs with Moveable) | Use, vendored |
| **@scena/guides** / react-guides | 0.29.2 (2023-07) | MIT | Rulers and guides | Use or build in-house |
| **interact.js** | 1.10.28 (**2026-08-01**) | MIT | Low-level drag, resize, gestures, snapping | **Fallback / foundation** if we replace Moveable with in-house handles |
| react-rnd | 10.5.3 (2026-03) | MIT | Simple drag/resize (React) | Too simple (no rotation or multi-select) |
| Konva / react-konva | 10.9.0 (2026-10) | MIT | Canvas scene graph + Transformer | Not for the main canvas (see B.15) |
| Fabric.js | 7.4.0 (2026-05) | MIT | Canvas object editor | Same reason |
| leafer-ui + @leafer-in/editor | 2.3.1 (2026-10) | MIT | Canvas engine + editor | Same reason |
| **GrapesJS** | 0.23.8 (2026-10) | BSD-3 | HTML template/page builder (CMS), with commercial Studio SDK ([repo](https://github.com/GrapesJS/grapesjs)) | No. Flow-layout web pages, not a broadcast canvas |
| Craft.js | 0.2.12 (2025-02) | MIT | React page builder | No (flow layout, slowing activity) |
| Puck (`@puckeditor/core`, formerly `@measured/puck`) | 0.23.0 (2026-08) | MIT | React visual editor (flow) | No for the canvas. Possibly useful for *settings-panel* layouts |
| **tldraw** | 5.5.2 (2026-10) | Source-available. **License key required in production; watermark otherwise** ([license key](https://tldraw.dev/sdk-features/license-key)) | Infinite canvas SDK | **Avoid** (licensing; whiteboard model) |
| Excalidraw | 0.18.1 | MIT | Whiteboard | No |
| Polotno | 4.16.1 | Commercial | Canvas design editor | No |
| **Motion** | 14.3.0 (2026-10) | MIT | Animation (WAAPI-backed, springs) | **Use** |
| anime.js | 4.5.0 (2026-06) | MIT | Animation | Alternative |
| GSAP | 3.15.0 (2026-04) | Standard no-charge (Webflow) | Animation | **Avoid** in core (prohibited-use clause) |
| **@lottiefiles/dotlottie-web** | 0.81.0 (2026-10) | MIT | Lottie/dotLottie (WASM renderer) | **Use** (lottie-web 5.13.0 last released 2025-05) |
| @rive-app/canvas | 2.45.0 (2026-10) | MIT | Rive runtime | Optional element |
| pixi.js | 8.22.0 (2026-10) | MIT | Particles / effects element | Use for the canvas element type |
| @xzdarcy/react-timeline-editor | 1.0.0 (2026-01) | MIT | Keyframe timeline UI | Use if React, otherwise in-house |
| Theatre.js | 0.7.2 (2024-05) | core Apache-2.0 / studio **AGPL-3.0** | Timeline studio | Avoid |
| **dockview** | 8.6.0 (2026-10) | MIT | IDE-style dockable panels for the editor shell | Use |
| **Zod** | 4.6.7 (2026-10) | MIT | Schema + JSON Schema export | Use (or TypeBox 0.34, MIT) |
| Ajv | 8.20.0 | MIT | JSON Schema validation (plugins, imports) | Use |
| **Yjs** | 13.6.33 (2026-09) | MIT | CRDT for undo/redo and future multi-user (mod co-editing) | Use (or immer patches if single-user only) |
| **DOMPurify** | 3.4.17 (2026-10) | MPL-2.0 OR Apache-2.0 | Sanitize HTML in user-authored rich text | Use |
| @twemoji/api | 17.0.3 (2026-06) | MIT + CC-BY-4.0 (graphics) | Emoji images | Use (attribution required) |
| emoji-regex | 11.0.0 (2026-09) | MIT | Emoji detection (emote-only messages) | Use |
| obs-websocket-js | 5.0.8 (2026-06) | MIT | OBS video settings, screenshots for the editor backdrop | Use |
| @twurple/eventsub-ws, @twurple/chat | 8.2.0 (2026-09) | MIT | Twitch EventSub (if the core is Node/TS) | Candidate |

**Editor UI framework:** Moveable, Motion, dotLottie and dockview all have vanilla cores, so React, Svelte 5 or Solid all work. The **runtime** should stay small (vanilla TS or Solid/Lit, no React) to keep CEF memory low across many browser sources.

## B.17 Licensing notes for code reuse

| Project | License | Can we copy code? |
|---|---|---|
| Firebot, Social Stream Ninja | GPL-3.0 | Only if Streamer.live is GPL-3.0. Otherwise reimplement from docs/behaviour |
| Kruiz Control | CC BY-NC-ND 4.0 | **No** (no derivatives, no commercial use) |
| Chatterino | MIT | Yes, with attribution (C++, so mainly ideas) |
| Mix It Up | Open source **(verify license)** | Check before reusing |

## B.18 Open questions for the project owner

1. **Desktop shell and WYSIWYG fidelity:** Electron (Chromium, same engine as OBS CEF) or Tauri (WebKitGTK on Linux, WebKit on macOS)? With Tauri, the in-app preview would **not** be pixel-identical to OBS. That could be handled by previewing in an OBS dock or a Chromium window.
2. **Editor UI framework:** React, Svelte or Solid? This decides react-moveable or vanilla Moveable, and the timeline component.
3. **Template syntax:** `{{var}}` (Lumia/Vision-compatible) or the Streamer.bot-style `%var%` used by our action engine? Or accept both?
4. **SE custom-widget compatibility shim in v1?** It brings a large ecosystem cheaply, but it is extra surface to maintain.
5. **DOM-capture ingestion** (SSN-style, for TikTok and long-tail platforms): in scope as an optional plugin, given ToS and fragility risk?
6. **Kick deleted-message sync** needs Kick's unofficial chat websocket, because the official webhooks have no delete event. Is that acceptable?
7. **Audio routing:** alert and TTS audio through the OBS browser source (mixable in OBS, needs "Control audio via OBS"), through app-native output (works without OBS), or user's choice per lane?
8. **Network exposure:** localhost only, or an authenticated LAN mode for dual-PC setups and the phone control deck?
9. **Queue defaults:** one global lane, or `main` + `minor` + `tts` lanes? Should higher-priority alerts interrupt by default? What should the default gift-bomb display be (suppress children, list recipients, or mini-ticker)?
10. **Sharing and marketplace:** local `.slpack` import/export only, or a hosted gallery? Signed packs?
11. **Project license:** MIT/Apache or GPL? This decides whether Firebot or SSN code can be reused.
12. **Collaboration:** is real-time mod co-editing a goal? If so, adopt Yjs from day one.
13. **Vertical canvas:** should every overlay support a 9:16 variant (Aitum Vertical users), or should it be opt-in?
14. **Paid features:** will anything be gated (Lumia gates alert variations), or will everything be free like Firebot and SSN?
