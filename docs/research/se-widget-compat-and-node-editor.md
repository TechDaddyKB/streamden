# StreamElements widget compatibility, and a node-based action editor

**Date:** 2026-10-18
**Scope:**
- **Part A:** run purchased StreamElements (SE) custom widgets natively in Streamer.live, with no SE account, fed by our combined multichat. Includes a Streamlabs second layer, prior art, legal notes, a runtime design, and the SE overlay editor's mental model.
- **Part B:** a node-graph view of our structured sub-action tree.

**Builds on:** [competitors-and-designer.md](./competitors-and-designer.md), which already lists SE, 1UP Vision and SSN at a high level. It also defines §B.10 (sandboxed custom widgets), §B.9 (alert lanes) and §B.14 (overlay runtime URL/WS protocol). This document goes deeper and refines B.10 where noted. Platform payload facts come from [platform-apis.md](./platform-apis.md), sub-action semantics from [streamerbot-engine-ui.md](./streamerbot-engine-ui.md), and plugin manifests from [plugin-sdk-and-api.md](./plugin-sdk-and-api.md).

**Method:**
- Official SE docs, read from their source repo [`StreamElements/docs`](https://github.com/StreamElements/docs) (Astro/Starlight, pushed 2026-10-14) and the older [`StreamElements/widgets`](https://github.com/StreamElements/widgets) repo.
- Community type definitions and emulators, plus prior-art apps.
- A static scan of **138 public widget units from 16 GitHub repos** (§A1.15). Nothing was executed.

**Clean-room note:**
- This document describes interfaces and observed behaviour in our own words and tables.
- No SE code is reproduced. SE's sample widgets carry **no license** ([repo](https://github.com/StreamElements/widgets): `license: null`), so they must not be copied into our repo or bundled as fixtures.

**Markers:**
- **(unverified)**: no primary source found.
- **(observed)**: seen in community code or captures, not in official docs.

---

## TL;DR

1. **Feasible, and high-value.** The SE custom-widget contract is small:
   - 3 window events and about 18 listener names.
   - 8 `SE_API` methods.
   - A fields JSON with 12 types.
   - `{{field}}`/`{field}` text substitution.
   - It is publicly documented, and community tooling has reverse-documented the rest.

   Several products already emulate it: 1UP Vision, SSN's importer, Sound Alerts, SE2SB (a Streamer.bot bridge), se-dev-env and sigmanuts.
2. **Expected compatibility (estimate):**
   - **≈85–90%** of typical Etsy *chat-box* widgets should run unmodified for Twitch chat.
   - **≈70–80%** should render YouTube/Kick lines acceptably through our Twitch-shaped multichat mapping.
   - Most breakage comes from widgets that call SE's cloud services (loyalty points, SE bot, JWT REST). It does not come from the widget API itself.
3. **The non-obvious requirements:**
   - Synthesize **Twitch IRC `tags`** from EventSub. Widgets read `tags['room-id']`, `mod`, `subscriber`, `badges`, `vip`, `msg-id` and `reply-parent-*` heavily.
   - Emit the **observed** button shape: `listener:"event:test"` with `event.listener:"widget-button"`. The official docs say otherwise.
   - Shim the SE REST `channels/{id}` (provider lookup) and `speech?voice=` (TTS) endpoints.
   - Run each widget on its **own real origin**, not an opaque `sandbox` origin, so `localStorage` and CORS behave as they do on SE.
   - Load `<script src>` tags **in order** before running the JS tab.
4. **Market context changed in 2026:**
   - SE announced it was closing on 2026-05-14 and reversed on 2026-05-21 ([GamesBeat](https://gamesbeat.com/streamelements-prepares-to-shut-down-website-and-creator-platform/)).
   - Razer bought SE's assets on 2026-07-31 ([Game Developer press release](https://www.gamedeveloper.com/press-release/razer-acquires-streamelements-assets-to-expand-content-creator-ecosystem)), and new SE Terms took effect 2026-08-01 ([terms](https://streamelements.com/terms)).
   - Streamers are actively looking for migration paths. One OSS renderer exists for exactly this "migrationary period" ([OpenElements](https://github.com/Raptosaur/OpenElements)).
5. **Streamlabs:**
   - Its Chat Box and Custom Widget use a *different* contract: `document`-level events, a server-rendered `#chatlist_item` template, and IRC-like `detail.command`.
   - It is poorly documented. Plan it as a **second layer on the same iframe substrate**, after SE.
6. **Node editor:**
   - Use **React Flow (`@xyflow/react` 12.12.0, MIT)** with **elkjs 0.12.0 (EPL-2.0)** for layout.
   - The graph is a **view** of the canonical tree:
     - Exec edges form single-entry/single-exit regions.
     - Control nodes (If/Switch/While/Group/Parallel) own their regions and expose an explicit "Then/Done" continuation port.
     - Data edges are typed references to earlier outputs.
     - Pure nodes (no exec pins) compile to inline expressions.
   - Avoid litegraph: the maintained fork moved into GPL-3.0 ComfyUI_frontend, and `@comfyorg/litegraph` is deprecated.

---

# Part A — StreamElements custom widget compatibility

## A1. The SE custom-widget API surface (exhaustive)

### A1.1 Anatomy: five parts and where they live

| Part | Editor tab | Content | Notes |
|---|---|---|---|
| HTML | HTML | Any HTML. `<script src>` and `<link>` to CDNs are allowed | [widget-structure](https://docs.streamelements.com/overlays/widget-structure) |
| CSS | CSS | Any CSS, including `@import url(...)`, animations and transitions | same |
| JS | JS | Plain JS that runs "in protected sandbox": no cookies, no `console.*`, no IndexedDB | same; [custom-widget](https://docs.streamelements.com/overlays/custom-widget) |
| Fields | FIELDS | JSON object `{ key: {type,label,value,group,…} }`. Rendered as the left-panel form | same |
| Data | DATA | JSON object of *saved field values* (`fieldData` overrides). Empty `{}` means defaults apply | (observed) `data.json` is `{}` in [pronouns-chat](https://github.com/liquidnya/pronouns-chat/tree/main/streamelements). In the overlay JSON it is `variables.fieldData` ([streamwizard plan](https://github.com/streamwizard/streamwizard/blob/main/docs/streamelements-import-plan.md), MIT) |

**Storage on SE's side.** In the overlay document returned by `GET /kappa/v2/overlays/{channel}/{overlayId}`:
- A custom widget is a `widgets[]` entry with `css` (`top/left/width/height/z-index/opacity`).
- Its `variables.html/css/js` hold the code.
- `variables.fields` holds the fields JSON as a **string**, and `variables.fieldData` holds the values.
- Sources: [streamwizard plan §1](https://github.com/streamwizard/streamwizard/blob/main/docs/streamelements-import-plan.md) and the official WS topic [`channel.overlay.update`](https://docs.streamelements.com/websockets/topics/channel-overlay-update), which shows the widget array shape.

### A1.2 Placeholder substitution

| Rule | Behaviour | Source |
|---|---|---|
| Syntax | `{{key}}` **and** `{key}` are equivalent | [widget-structure](https://docs.streamelements.com/overlays/widget-structure), [alertbox](https://docs.streamelements.com/overlays/custom-code-in-alertbox) |
| Where | HTML, CSS **and JS**. SE suggests HTML/CSS only "for readability", but real widgets use it in JS, e.g. `parseFloat("{{someNumber}}")` or class names inside JS template strings | same; SE's own CustomChat sample uses `{animationIn}` and `{emoteSize}` inside JS (observed) |
| When | Text substitution of the source **before** execution | (observed) [se-dev-env runtime](https://github.com/Redo7/se-dev-env/blob/main/templates/iframe/script.js) replaces `{{x}}` and then `{x}` |
| Unknown names | Not documented. Emulators leave unknown tokens intact | (observed) se-dev-env |
| Escaping | None documented. Values are inserted raw, so a quote in a text field can break JS | (unverified) |
| Types | Booleans and numbers stringify. `multiple:true` media fields are arrays, so the string form is comma-joined | (unverified) |
| AlertBox custom code | A different variable set: `{{name}} {{amount}} {{tier}} {{sender}} {{currency}} {{items}} {{announcement}}/{{messageTemplate}} {{message}}/{{userMessage}} {{messageRaw}} {{image}} {{video}} {{videoVolume}} {{audio}} {{audioVolume}} {{widgetDuration}}` | [custom-code-in-alertbox](https://docs.streamelements.com/overlays/custom-code-in-alertbox) |

### A1.3 Field types

| `type` | Value | Extra attributes | Corpus count* | Source |
|---|---|---|---|---|
| `text` | string | — | 1087 | [widget-structure](https://docs.streamelements.com/overlays/widget-structure) |
| `hidden` | any | not shown; used for reserved fields | 995 | same |
| `slider` | number | `min` `max` `step` | 909 | same |
| `dropdown` | option **key** | `options: {"key":"Label"}` | 853 | same |
| `number` | number | `min` `max` `step` | 738 | same |
| `colorpicker` | CSS color string. Hex **or rgba**, used verbatim in CSS | — | 679 | same; rgba seen in SE's CustomChat fields (observed) |
| `sound-input` | URL, or array if `multiple:true` | `multiple` | 210 | same |
| `checkbox` | boolean | — | 179 | same |
| `image-input` | URL or array | `multiple` | 167 | same |
| `video-input` | URL or array | `multiple` | 75 | same |
| `googleFont` | font family name | — | 61 | same |
| `button` | label/value. A click emits a button event (A1.7) | — | 48 | same |
| other HTML5 input types (`password`, `date`, `url`, `email`…) | string | — | 6 (`password`) | "most of HTML5 input types (except file)" |
| invalid/unknown | **falls back to `text`** | — | — | same |

\* Occurrences of `"type":"…"` across the corpus (§A1.15). `textarea` is **not documented**, and `fontpicker` is a Streamlabs type.

**Common attributes:**
- `type`, `label`, `value`.
- `group` (fields sharing a group name form a collapsible section).
- Key **order** in the JSON is the display order. Our parser must preserve it, and `serde_json` needs its `preserve_order` feature.

**Reserved field names.** All future reserved names will start with `widget`.
- `widgetName` (display name), `widgetAuthor` (shows "(by X)"), `widgetDuration` (queue hold, in seconds).
- Seen in SE's own samples: `widgetVersion`, `widgetUpdateUrl` (observed).

### A1.4 Runtime environment

| Aspect | SE behaviour | Implication for us |
|---|---|---|
| jQuery | "jQuery is included by default" ([events](https://docs.streamelements.com/overlays/events)). Version not documented (unverified) | Inject jQuery 3.7.x (MIT) by default, before widget code. 45% of corpus units use `$` |
| External libs | Any `<script src>` or `<link>` is allowed. The docs' own example loads jQuery UI from cdnjs, and Google Fonts via `<link>`/`@import` | Allow outbound network by default (as SE does), with an import-time host report |
| Restrictions | No `document.cookie`, IndexedDB or `console.*` ([widget-structure](https://docs.streamelements.com/overlays/widget-structure)) | We can be more permissive. Console goes to our dev panel |
| Globals | `SE_API`, `$`/`jQuery`. Emulators also set `fieldData`, `channel`, `currency` on `window` (observed in se-dev-env; unverified on SE) | Provide `SE_API` and jQuery. Optionally mirror `window.fieldData` |
| Script order | The JS tab must run after HTML `<script src>` libraries load, or widgets using GSAP etc. crash (observed: emulators await each script in order) | Our loader awaits each external script in document order (A6.4) |
| Overlay token | The overlay URL contains a token, and `channel.apiToken` is given to widgets | **Never** give widgets a real token. Use `apiToken: ""` |

### A1.5 `onWidgetLoad` — `obj.detail`

| Key | Shape | Source |
|---|---|---|
| `channel` | `{username, apiToken, id (SE 24-hex id), providerId (e.g. Twitch user id), avatar}` | docs list `username`, `apiToken`. `id`, `providerId`, `avatar` are (observed) in a real capture ([se-dev-env onWidgetLoad.json](https://github.com/Redo7/se-dev-env/blob/main/data/onWidgetLoad.json)) |
| `currency` | `{code:"USD", name:"U.S. Dollar", symbol:"$"}` | [events](https://docs.streamelements.com/overlays/events) |
| `fieldData` | merged field values | same |
| `recents` | last 25 events, chronological, items like `{name, amount?, tier?, createdAt, type}` | same; [session-data](https://docs.streamelements.com/overlays/session-data) |
| `session.data` | ~105 keys (A1.6) | same |
| `session.settings` | `{autoReset, calendar, resetOnStart}` | (observed) capture |
| `overlay` | `{isEditorMode, muted}` | (observed) capture; same as `SE_API.getOverlayStatus()` |
| `emulated` | boolean when fired by the editor | (observed) [Tixyel types](https://github.com/Tixyel/widgets/blob/main/packages/client/src/types/streamelements/main.ts) (Apache-2.0) |

### A1.6 `session.data` keys

Source: [session-data](https://docs.streamelements.com/overlays/session-data) and [GitHub docs](https://github.com/StreamElements/widgets/blob/master/docs/CustomWidgetEvents.md). A real capture had 105 keys.

| Family | Keys (P = `session|weekly|monthly|alltime`) | Fields |
|---|---|---|
| Follower | `follower-latest`, `-session`, `-week`, `-month`, `-total`, `-goal`, `-recent` | `name` / `count` / `amount` / array |
| Subscriber (Twitch, Kick) | `subscriber-latest` (`name, amount=months, tier 1–3, message, sender, gifted`), `subscriber-new-latest`, `subscriber-resub-latest`, `subscriber-gifted-latest`, `subscriber-alltime-gifter`, `subscriber-{new,resub,gifted}-session`, `subscriber-{session,week,month,total}`, `-goal`, `-points`, `-recent` (tier 1000/2000/3000) | |
| Cheer | `cheer-latest`, `cheer-{session,week,month,total}`, `cheer-count`, `cheer-goal`, `cheer-P-top-donation`, `cheer-P-top-donator`, `cheer-recent` | donation = largest single, donator = per-user sum |
| Tip | same pattern as cheer, plus `tip-count`, `tip-goal`, `tip-recent` | |
| Raid / Host | `raid-latest`, `raid-recent`, `host-latest`, `host-recent` | `amount` = viewers |
| Merch / Purchase | `merch-goal-{items,orders,total}`, `merch-latest` (items[]), `merch-recent`, `purchase-latest` | |
| Channel points, Hype Train | `channel-points-latest` (`redemption`), `hypetrain-latest`, `-level-goal`, `-level-progress`, `-total`, `-latest-top-contributors` | |
| Charity, cheerPurchase | `charityCampaignDonation-*`, `cheerPurchase-*` (P-top patterns, `-recent`) | |
| KICKs (Kick, since 2026-09-13) | `kicks-latest`, `kicks-{session,month,total}`, `kicks-count`, `kicks-goal`, `kicks-P-top-*` | [changelog](https://github.com/StreamElements/docs/blob/master/src/content/changelog/dashboard/2026-09-13-kicks-support.mdx) |
| YouTube | `sponsor-*` (memberships, legacy name), `sponsor-gifted-latest`, `superchat-*` (incl. P-top). Super Stickers count as superchats since 2026-09-17 | [changelog](https://github.com/StreamElements/docs/blob/master/src/content/changelog/dashboard/2026-09-17-youtube-super-stickers-and-jewels.mdx) |
| Facebook (legacy) | `fan-*`, `share-*`, `stars-*`, `supporter-*`, `videolike-*` | GitHub docs only |

Session behaviour: sessions auto-reset 15 min after going offline. Goals do not reset ([widget-data](https://docs.streamelements.com/overlays/widget-data)).

### A1.7 `onEventReceived` — listener values

`obj.detail = {listener, event}` ([events](https://docs.streamelements.com/overlays/events)).

| Listener | Status | `event` payload |
|---|---|---|
| `message` | doc | `{service, data:{…}, renderedText}` (A1.8) |
| `delete-message` | doc | `{msgId}` |
| `delete-messages` | doc | `{userId}` |
| `follower-latest` | doc | `{name, displayName, providerId, avatar, _id, sessionTop, type:"follower", originalEventName}` (fields beyond `name` observed) |
| `subscriber-latest` | doc | `{name, displayName, amount, tier:"1000"/"2000"/"3000"/"prime", message, gifted, sender, bulkGifted, isCommunityGift, playedAsCommunityGift, …}` (A1.9) |
| `cheer-latest` | doc | `{name, amount, message, …}` |
| `tip-latest` | doc | `{name, amount, message, …}`. `userCurrency` mentioned for donations |
| `raid-latest` / `host-latest` | doc | `{name, amount=viewers}`. Twitch removed hosting, so `host` never fires in practice |
| `kicks-latest` | doc (2026) | `{name, amount, message}`, "shaped like the Twitch cheer keys" |
| `superchat-latest`, `sponsor-latest` | (observed) | YouTube super chat, and membership with the same gift flags as subs ([Tixyel YouTube types](https://github.com/Tixyel/widgets/blob/main/packages/client/src/types/streamelements/events/youtube.ts)) |
| `merch-latest`, `purchase-latest`, `charityCampaignDonation-latest` | (unverified) as listeners | session keys exist, and the AlertBox `listeners` map contains `purchase-latest` ([overlay.update](https://docs.streamelements.com/websockets/topics/channel-overlay-update)) |
| `event` | (observed) | raw activity `{type, provider, data:{…}, createdAt, _id, activityId, …}`. Used for e.g. `channelPointsRedemption` ([devkit types](https://github.com/LiamDochartaigh/streamelements-devkit/blob/main/src/se-types.d.ts)). It is in SE's own "skippable" list |
| `event:test` | (observed) | carries **widget-button clicks** as `{listener:"widget-button", field, value}` and editor emulations |
| `widget-button` | doc | docs say `obj.detail.listener === 'widget-button'` with `{field, value}`. **But** SE's own CustomChat sample checks `obj.detail.event.listener === 'widget-button'`, and two independent community typings put it under `event:test`. Treat the observed form as real |
| `event:skip` | doc | `{}`. The streamer pressed skip |
| `alertService:toggleSound` | doc | `{muted}` (observed field) |
| `bot:counter` | doc | `{counter, value}` |
| `kvstore:update` | doc | `{data:{key:"customWidget.<key>", value}}` |

Corpus usage among 84 chat-capable units (§A1.15): `widget-button` 20%, `event:test` 14%, `delete-message(s)` 7%, `bot:counter` 10%.

### A1.8 Chat `message` payload

**Twitch** (`event.service === "twitch"`). Fields are documented unless marked (observed).

| Field | Type / example | Notes |
|---|---|---|
| `data.time` | ms epoch | |
| `data.tags` | IRC tag map (strings): `badges` ("broadcaster/1,subscriber/12"), `badge-info`, `color`, `display-name`, `emotes` ("25:5-9"), `flags`, `id`, `mod` ("0"/"1"), `room-id`, `subscriber`, `tmi-sent-ts`, `turbo`, `user-id`, `user-type`. Observed extras: `first-msg`, `returning-chatter`, `client-nonce`, `reply-parent-*`, `msg-id` | **Heavily used** (A1.15) |
| `data.nick` | login | |
| `data.userId` | Twitch user id | |
| `data.displayName`, `data.displayColor` | | `displayColor` may be `""` |
| `data.badges[]` | `{type, version, url, description}` | URL = Twitch badge CDN, 3x |
| `data.channel` | channel login | |
| `data.text` | raw text | |
| `data.isAction` | `/me` | |
| `data.emotes[]` | `{type:"twitch"\|"bttv"\|"ffz"\|"7tv", name, id, gif, urls:{"1","2","4"}, start, end}` | Third-party emotes appear to lack `start/end`, so widgets match them by name (observed) |
| `data.msgId` | message id | needed for deletions |
| `renderedText` | HTML with `<img class="emote" …>` substituted | (observed) in SE CustomChat and typings. Not in official docs |
| `service` | `"twitch"`/`"youtube"`/… | (observed) |

**YouTube** (`service:"youtube"`):
- Contains the raw YouTube `liveChatMessage` (`kind`, `etag`, `id`, `snippet{type, liveChatId, authorChannelId, publishedAt, displayMessage, textMessageDetails}`, `authorDetails{channelId, displayName, profileImageUrl, isVerified, isChatOwner, isChatSponsor, isChatModerator}`).
- **Plus** the Twitch-like fields: `msgId`, `userId`, `nick`, `displayName`, `text`, `emotes`, `time`, `channel`, `avatar`.
- SE leaves `badges: []`, `tags: []` and `displayColor: null` ([events](https://docs.streamelements.com/overlays/events)).
- So on SE, Twitch-designed widgets already see *degraded* YouTube lines: no badges, no colour.

**Kick:** SE's message payload for Kick chat is **not documented** (unverified). SE does support Kick follows, subs and KICKs.

### A1.9 Sub/gift flag matrix (`subscriber-latest`, mirrored by `sponsor-latest`)

| Case | `name` | `sender` | `gifted` | `bulkGifted` | `isCommunityGift` | `amount` |
|---|---|---|---|---|---|---|
| New/resub | subscriber | — | false | false | false | months |
| Directed gift | recipient | gifter | true | false | false | months |
| Community gift (initial) | gifter (observed `name === sender`) | gifter | false | **true** | false | gift count |
| Community gift (each recipient) | recipient | gifter | true | false | **true** | months |
| Played as part of cumulative alert | — | — | — | — | — | `playedAsCommunityGift: true` |

Sources: [events](https://docs.streamelements.com/overlays/events) and [Tixyel Twitch types](https://github.com/Tixyel/widgets/blob/main/packages/client/src/types/streamelements/events/twitch.ts).

### A1.10 `onSessionUpdate`

`obj.detail.session` **is** the data object. In `onWidgetLoad` it is at `obj.detail.session.data`, so the paths differ ([events](https://docs.streamelements.com/overlays/events)). It fires only when session stats change. It is recommended for goals and "top donator changed".

### A1.11 `SE_API`

| Method | Contract | Source |
|---|---|---|
| `store.set(key, obj)` | Global per SE account. Overwrites the whole object (no partial update). Keys cannot be deleted. Broadcasts `kvstore:update` to all custom widgets | [custom-widget](https://docs.streamelements.com/overlays/custom-widget) |
| `store.get(key)` | Promise → object | same |
| `counters.get(name)` | Promise → `{id, count}` (new docs) / `{counter, value}` (old docs) | same; [GitHub docs](https://github.com/StreamElements/widgets/blob/master/docs/CustomWidget.md) |
| `sanitize({message})` | Promise → `{skip, result:{message}}` using the account's banned-word rules | same |
| `cheerFilter(message)` | Promise → string without cheermotes | same |
| `getOverlayStatus()` | Promise → `{isEditorMode, muted}` | same |
| `setField(key, value, shouldReload=true)` | Editor-only, unsaved | same |
| `resumeQueue()` | Releases the queue hold early | same |
| `sendMessage`, `responses` | Undocumented internals (observed in typings) | stub |

**Queue semantics:**
- `widgetDuration` (hidden field, seconds, default 0) is the **maximum time a widget holds the overlay's event queue** after receiving a queued alert-type event. `resumeQueue()` releases it early.
- SE's example lists *skippable* listeners that bypass the hold: `bot:counter`, `event`, `event:test`, `event:skip`, `alertService:toggleSound`, `message`, `delete-message`, `delete-messages`, `kvstore:update` ([custom-widget](https://docs.streamelements.com/overlays/custom-widget)).
- AlertBoxes have their own timers.

### A1.12 AlertBox custom code vs Custom Widget

| | AlertBox "custom CSS" per alert type/variation | Custom Widget |
|---|---|---|
| Scope | One template per event type/variation, rendered per alert | Persistent page that receives every event |
| Data | Inline variables (A1.2 last row) | Events + `SE_API` |
| Queue | AlertBox timers | `widgetDuration` + `resumeQueue` |
| Our mapping | **Our alert variation template** (designer §B.8): import as an alert template with HTML/CSS/JS, mapping `{{name}}` etc. to our bindings | **SE Widget Runtime** (A6) |

### A1.13 Emote and badge sources

| Source | How widgets get it |
|---|---|
| Twitch emotes | `data.emotes[].urls` (SE uses the legacy `emoticons/v1/{id}/{1.0,2.0,3.0}` CDN). Current CDN template: `static-cdn.jtvnw.net/emoticons/v2/{id}/{format}/{theme}/{scale}` ([Helix Get Emotes](https://dev.twitch.tv/docs/api/reference/#get-global-emotes)) |
| Twitch badges | `data.badges[].url` |
| BTTV/FFZ/7TV | Delivered in `emotes[]` by SE ([chat widget doc](https://docs.streamelements.com/overlays/chat-widget) says the SE chat renders them, except unlisted 7TV). 8% of corpus chat units also fetch 7TV/BTTV/FFZ APIs themselves using `channel.providerId` |
| Cheermotes | `renderedText`, AlertBox `{{message}}` (`.alertbox-message-emote`) |
| Avatars | Not in Twitch message payloads. Widgets call `decapi.me` (10% of chat units) or the SE API. YouTube payloads carry `avatar` |
| Pronouns | `api.pronouns.alejo.io`, `pronoundb.org` (observed in pronouns-chat) |

### A1.14 Other SE surfaces relevant to import

**Overlay WS topics:**
- `channel.overlay.update` returns the full overlay JSON.
- `channel.overlay.action` carries `pause, unpause, mute, unmute, skip, playNext, emptyHypeCup, reload`.
- New 2026-03-17: `channel.overlay.broadcast`, a custom event name (≤256 chars) plus a payload (≤5 KB) targeting all overlays or one. SE's own `!kappagen` uses it.

Sources: [update](https://docs.streamelements.com/websockets/topics/channel-overlay-update), [action](https://docs.streamelements.com/websockets/topics/channel-overlay-action), [broadcast](https://docs.streamelements.com/websockets/topics/channel-overlay-broadcast). How broadcast events surface inside custom widgets is not documented (unverified).

**Overlay geometry:**
- Absolute px inside `settings.width × height`.
- Groups are a flat `se-widget-group` with a `uid` that children reference.
- No rotation or crop ([streamwizard plan](https://github.com/streamwizard/streamwizard/blob/main/docs/streamelements-import-plan.md)).

### A1.15 Corpus analysis: what real widgets actually use

**Method:**
- Downloaded tarballs of 16 public repos: StreamElements/widgets, swolekat/se-widgets, jhoooooo/chatbox, reboot0-de/se-tools, liquidnya/pronouns-chat, FabioZumbi12/StreamElements-TwitchChat, GlitchedMythos/se-widgets, brofar/StreamElements-Shoutout, c4ldas/streamelements-widgets, j3channel/StreamElementsWidgets, MrAdamskyyS, pjonp/pjonpWidgets, JaLDesign, neoshinyt/Neohud, mr-shade/megathon-widget, Linedev-Web/OBS.
- A "unit" is a directory whose files add `onWidgetLoad`/`onEventReceived` listeners. This found **138 units, 84 with a `message` listener**. "Chat-capable" includes chat-command games, not only chat boxes.
- Regex scan only. **Bias:** GitHub widgets are more developer-heavy than Etsy chat boxes, which are mostly cosmetic.

| Feature (chat-capable units, n=84) | % |
|---|---|
| Google Fonts | 63 |
| `{{field}}` in code | 44 |
| jQuery `$` | 40 |
| `fetch`/XHR of any kind | 46 |
| `badges[]` | 33 |
| IRC `tags` (`room-id` 47 refs, `mod` 40, `subscriber` 28, `badges` 28, `display-name` 10, `vip` 8, `msg-id` 5, `reply-parent-*`, `badge-info`) | 32 |
| `{field}` single-brace in JS strings | 21 |
| `widget-button` | 20 |
| `api.streamelements.com` | 19 |
| `displayColor` | 17 |
| `channel.id` | 14 |
| `event:test` | 14 |
| `service`/`provider` checks (`provider==="twitch"`, legacy `"mixer"`) | 14 |
| `apiToken` | 13 |
| `animate.css` | 13 |
| `SE_API.store.get/set` | 13 |
| `renderedText` | 12 |
| GSAP | 11 |
| `SE_API.resumeQueue` | 11 |
| TTS (`speechSynthesis` or SE speech API) | 11 |
| `bot:counter` | 10 |
| `decapi.me` | 10 |
| WebSocket | 8 |
| 7TV/BTTV/FFZ APIs | 8 |
| `delete-message(s)` | 7 |
| `widgetDuration` | 6 |
| `eval`/`new Function` | 5 |
| `onSessionUpdate` | 1 |
| No network beyond static CDNs | **45** |
| Network deps (apiToken / SE REST / WS / decapi / Helix) | 38 |

**SE endpoints referenced:**

| Endpoint | Units | Purpose |
|---|---|---|
| `kappa/v2/channels/{id}` | 6 | Provider lookup. SE's own CustomChat does this |
| `kappa/v2/speech?voice=` | 6 | Public TTS |
| `kappa/v2/points/…` | 3 | Loyalty |
| `kappa/v2/bot/…` | 3 | Bot commands / say |
| `realtime.streamelements.com` | 1 | Legacy socket |

**Top external hosts:**
- fonts.googleapis.com, cdnjs.cloudflare.com, cdn.jsdelivr.net, static-cdn.jtvnw.net.
- swolekat.github.io (author-hosted assets), decapi.me, cdn.7tv.app, unpkg.com.

The scripts and per-widget results live in `/tmp/se-research/` and are not part of the repo.

---

## A2. How Etsy and store sellers deliver widgets

| Format | How it looks | Can we ingest it without SE? |
|---|---|---|
| **Five files** in a ZIP | `widget.html/css/js` + `fields.json` + `data.json`. Also seen: `html.txt`, `css.txt`, `js.txt`, `fields.txt`, `data.txt`. SE's own repo names the fields file `widget.json` | **Yes**. Name and content sniffing (A6.2) |
| **widget.io export** | ZIP with `html.txt/css.txt/js.txt/fields.txt/data.txt` + a `widget.ini` mapping. Chrome extension for SE ([widget.io](https://github.com/sigma-cw/widget-io), MIT; [ini](https://github.com/Redo7/se-dev-env/blob/main/data/widget.ini)). Can minify or obfuscate | **Yes**. Parse the `.ini` |
| **Single text doc / PDF** | "Paste this into HTML tab… CSS tab…" install guides. A Scribd guide shows the pattern ([Scribd](https://www.scribd.com/document/703640449/Guide)) | **Mostly**. Section splitter plus smart-quote repair (A6.2) |
| **SE overlay share link** | `streamelements.com/dashboard/overlays/share/<overlayId>`. It POSTs `/kappa/v2/overlays/<destAccount>/share {overlayId}`, needs the owner's `share-overlay` permission flag, and installs into the **buyer's SE account** ([unofficial API notes](https://github.com/c4ldas/streamelements-api)) | **No**. Code is never public. Buyer must install into a (free) SE account, then copy the tabs or use our optional "Import from my SE account" (A6.2) |
| **Third-party quick links/codes** | e.g. [seapi.c4ldas](https://github.com/c4ldas/c4ldas-seapi) one-time codes that install via SE OAuth | Same as share links |
| **Streamlabs variant** | Chat Box HTML/CSS/JS + custom fields JSON, pasted into the Streamlabs dashboard ([pronouns-chat streamlabs](https://github.com/liquidnya/pronouns-chat/tree/main/streamlabs)) | Via the Streamlabs layer (A3) |

**Listing and licensing norms** (Etsy pages block automated fetch; facts are from search snippets):
- "personal use only… non-exclusive and can/will be sold to multiple customers" ([bubble chat listing](https://www.etsy.com/listing/1854755224/customizable-bubble-chat-widget-stream)).
- Some listings are "StreamElements Only And Twitch Only" ([listing](https://www.etsy.com/listing/1738148698/animated-twitch-chat-widget-interactive)).
- Some warn of limited YouTube badge/emote support.
- Multistream "Twitch YouTube Kick" chat listings exist.
- Typical deliverable: an instant-download PDF with links and code.
- Some sellers **lock widgets to a channel name** in code (unverified for Etsy specifically). We must pass the real username and never spoof it.

**Recommendation for buyers who only have a share link:**
1. Create a free SE account and click the link.
2. Open the widget, then **OPEN EDITOR**, and copy the 5 tabs (SE2SB documents exactly this; [README](https://github.com/imAreinieR/SE2SB-Overlay-Migrator)).
3. Paste them into our importer.

Alternatively, use our one-shot "Import from my SE account".

---

## A3. Streamlabs custom code (for a second compat layer)

| Surface | Contract | Source / status |
|---|---|---|
| **Chat Box custom HTML** | HTML must contain `#log` (`.sl__chat__layout`) and `<script type="text/template" id="chatlist_item">`. Streamlabs **renders** each message with tokens `{from}`, `{message}` (HTML incl. emotes), `{color}`, `{messageId}`, then appends it. Badges are added by Streamlabs into the badges span (unverified) | [pronouns-chat](https://github.com/liquidnya/pronouns-chat/blob/main/streamlabs/custom.html), [resir014 template](https://github.com/resir014/stream-overlays/blob/main/templates/chatbox/chatbox.html) |
| Chat Box JS events | `document.addEventListener('onEventReceived', e => e.detail)`. Detail is IRC-like `{command:"PRIVMSG", messageId, from, tags{…}, body, platform, payload{raw, prefix, params, tags}}`. Also `CLEARCHAT` (`tags['target-user-id']`, `body`=username) | (observed) [pronouns-chat handlers](https://github.com/liquidnya/pronouns-chat/tree/main/src/streamlabs/handlers). Known Streamlabs bug: messages with `<` and `>` get double-encoded |
| Chat Box custom fields | JSON `{name:{label, type, name, value, min, max, steps}}`, referenced as `{name}` | (observed) [custom.json](https://github.com/liquidnya/pronouns-chat/blob/main/streamlabs/custom.json) |
| **Custom Widget** (stored in a "widget theme", one per theme) | `document.addEventListener('onEventReceived', obj => obj.detail)` with `{type, name, from, platform:"youtube_account", message, amount, currency, displayString, isTest}`. **No built-in queue**. Platforms: Streamlabs tips/merch, Twitch, YouTube, Kick (follows, subs, KICKs), X, Patreon, Facebook | [Streamlabs guide](https://streamlabs.com/content-hub/post/how-to-get-started-with-streamlabs-custom-widgets) |
| **Alert Box** | Per-alert "Custom HTML/CSS/JS" toggle. Message templates accept HTML. Tokens by event: `{name}`, `{amount}`, `{months}`, `{gifter}`, `{count}`, `{product}`. Custom-HTML tokens such as `{messageTemplate}`, `{userMessage}`, `{img}` appear in community snippets only | [template params](https://streamlabs.com/content-hub/post/message-template-parameters-for-alert-box-widget-on-streamlabs-twitch-and-youtube); custom tokens (unverified) |

**Verdict:** feasible, at roughly 30–40% of the SE layer's effort, because it reuses the iframe runtime, fields engine and importer.

Main differences:
- We must **render the chat template ourselves** (token fill plus append to `#log`) *before* dispatching the IRC-like event, because widget JS post-processes the DOM.
- Event names live on `document` rather than `window`.
- Streamlabs field types differ (`fontpicker`, `steps`).

Do it after SE. Many Etsy products ship both versions, so SE coverage captures most purchases.

---

## A4. Prior art: running SE widgets outside SE

| Project | What it does | What works / breaks | License |
|---|---|---|---|
| **1UP Vision** (cloud) | SE compat layer: `onWidgetLoad`/`onEventReceived`/`onSessionUpdate`, all 7 `SE_API` methods. ZIP/file import with role detection (`html.txt`, `chat.html`, `style.css`, `widget.js`, `fields.json`, `data.json`/`fielddata.json`). Sandboxed iframe | Fields: only 9 types listed (no googleFont/sound/video). Store is global per account. No gap list published ([docs](https://docs.1up.vision/custom-widgets)) | proprietary |
| **SSN StreamElements importer** | Imports ZIP/files/folder → **single exported HTML** fed by SSN's relay. Maps `chatname→displayName/nick`, `chatmessage→text/renderedText`, `chatbadges→badges`, `type→service`, `mid→msgId`, donations→`amount` | "Goals, alerts, loyalty, store, tips, and account API features are only shimmed if their code falls back cleanly." Breaks on private SE/SL tokens, overlay-store state, "full jQuery plugins", platform emote APIs. Remote URLs stay remote ([importer](https://socialstream.ninja/streamelements-importer.html)) | GPL-3.0 (do not copy) |
| **Sound Alerts** | "Import Widget" for "compatible" SE widgets | No details ([help](https://support.soundalerts.com/article/Custom-Widget)) | proprietary |
| **Lumia Stream importer** | Imports SE overlays/alerts/commands/timers/loyalty using the user's **SE JWT**. Converts to native Lumia overlays, "supported widgets" only | Custom-widget support not stated ([page](https://lumiastream.com/services/streamelements)) | proprietary |
| **SE2SB Overlay Migrator** (Streamer.bot port, 2026) | C# desktop tool. Imports the 5 files, generates `streamerBotApiAndEventBridge.js` (SE REST → Streamer.bot WS, SB events → SE events, DecAPI fill-ins) + `sessionData.js`. `SE_API.store.set` → SB globals via helper actions | Only a subset of SessionData is real; the rest is zeros "so widget code won't throw". Upgrades `//` protocol-relative URLs to `https:` ([README](https://github.com/imAreinieR/SE2SB-Overlay-Migrator)) | MIT |
| **se-dev-env** | Local dev environment (React/Vite). Five-file templates, widget.io import/export, SE_API mock with persistent store, console bridge, field editors for every SE type | Our best reference for loader order and substitution ([repo](https://github.com/Redo7/se-dev-env)) | no license (read only) |
| **sigmanuts** (2023) | WebView2 app scraping YouTube chat into SE-shaped events (`message`, `member-latest`, `gift-latest`, `superchat-latest`, `sticker-latest`), served at `localhost:6969/widgets/...` | "Same widget structure as SE — any custom Twitch chat can be adapted **with a few tweaks**". Proves the multichat→SE-shape idea and its limits ([README](https://github.com/sigma-cw/sigmanuts-webview2), [API wiki](https://github.com/sigmacw/sigmanuts-webview2/wiki/Sigmanuts-API)) | MIT |
| **OpenElements** (2026) | Preact renderer for exported SE overlays (images, text, groups, alert box). WIP; custom widgets not yet | [repo](https://github.com/Raptosaur/OpenElements) | NOASSERTION |
| **streamwizard** plan (2026-09) | Detailed SE overlay import plan: token-based fetch, widget-type mapping, custom-widget compat table, session emulation, asset copy, never expose `apiToken` | Research only ([plan](https://github.com/streamwizard/streamwizard/blob/main/docs/streamelements-import-plan.md)) | MIT |
| **Tixyel/widgets** client | Typed SE events (Twitch/YouTube/SE), local emulator and queue | Best public typing of observed payloads ([types](https://github.com/Tixyel/widgets/tree/main/packages/client/src/types/streamelements)) | Apache-2.0 |
| **LiamDochartaigh/streamelements-devkit** | Local dev kit plus `se-types.d.ts` | Confirms the `event:test` + `widget-button` shape ([types](https://github.com/LiamDochartaigh/streamelements-devkit/blob/main/src/se-types.d.ts)) | no license |

**Lessons:**
- Everyone emulates the **same small surface**.
- Failures cluster around **SE cloud services** and **session data**.
- Nobody publicly maps **multi-platform** chat into a full Twitch-shaped payload with synthesized `tags`/badges. That is our differentiator.

---

## A5. Legal and ToS notes (not legal advice)

1. **Interface reimplementation:**
   - Re-implementing a published API from its documentation is the clean-room pattern courts have favoured.
     - **US:** *Google LLC v. Oracle America* (2021) held that Google's reimplementation of Java SE API declarations was fair use ([opinion](https://www.supremecourt.gov/opinions/20pdf/18-956_d18f.pdf)).
     - **EU:** *SAS Institute v. World Programming* (CJEU C-406/10, 2012) held that functionality, programming languages and data-file formats are not protected as such ([curia](https://curia.europa.eu/juris/liste.jsf?num=C-406/10)).
   - Our approach:
     - Implement from public docs and observed payloads.
     - Copy no SE code or assets (e.g. no SE default alert GIF/OGG, no SE sample widgets).
     - Use our own names internally.
2. **SE Terms** ([terms](https://streamelements.com/terms), updated 2026-08-01, StreamElements Pte. Ltd., Singapore law):
   - The relevant clauses forbid copying or modifying "any part of the Services" (§4(i)), automated access (§4(iv)), and reverse engineering the Services (§8.3(iii)). Users retain ownership of their submissions (§9.2).
   - A Streamer.live user who never uses SE is not bound by these terms.
   - Our runtime never touches SE services unless the *widget's own code* calls them (e.g. the public TTS endpoint). That traffic is the widget author's choice, and we offer local shims instead (A6.12).
   - The optional "Import from my SE account" uses the user's own token against SE's published API for the user's own data. This is low risk: use the official API, one shot, rate-limited, no dashboard scraping.
3. **Trademarks:**
   - Use nominative phrasing such as "Runs widgets made for StreamElements custom widgets" with a "not affiliated with StreamElements or Razer" disclaimer.
   - Do not use SE logos.
4. **Etsy/seller licenses** (typically personal use, single channel, no resale or redistribution):
   - Running the buyer's copy locally is consistent with these terms.
   - We must **not**:
     - host a public gallery of imported paid widgets;
     - sync widget code to any cloud, or send it to AI providers, without explicit consent (our AI-agent fleet must treat imported widget code as licensed third-party content);
     - circumvent seller channel locks.
   - Opportunity: a native package format with license metadata that sellers can publish directly (§Open questions).
5. **Fonts and CDNs (privacy):**
   - Google Fonts hot-linking has GDPR history (LG München I, 3 O 17493/20, 2022) **(unverified citation details)**.
   - Offer "cache fonts locally" for EU users.

---

## A6. Design: the "SE Widget Runtime"

### A6.1 Architecture

```
OBS browser source ──► http://127.0.0.1:P/o/<overlayId>?token=T          (overlay runtime, §B.14)
                         │  WS to core (events, state, alert lanes)
                         │
                         ├─ native layers (DOM renderer)
                         └─ SE layer  <iframe src="http://w-<hash>.localhost:P/sew/<instanceId>/?cap=C"
                                              sandbox="allow-scripts allow-same-origin allow-forms">
                              ├─ prelude.js  (SE_API shim, jQuery, fetch/XHR interceptor, localStorage shim, console bridge)
                              ├─ widget HTML (field-substituted), external <script>s loaded in order
                              ├─ widget CSS (field-substituted)
                              └─ widget JS  (field-substituted) → dispatch onWidgetLoad
                              ⇅ postMessage only (typed bridge; parent validates origin + instanceId)
Core (Rust): SeCompat module
  - import/normalize/lint  - substitution  - session-data aggregator (SE keys from our event store)
  - payload mappers (unified events → SE listener+payload)  - SE_API backends (KV, counters, sanitize, queue)
  - REST shim handlers (channels, speech→our TTS, bot say→our chat send)
```

### A6.2 Import pipeline

| Step | Detail |
|---|---|
| **Inputs** | (1) A **paste UI with 5 tabs** that mirrors SE (HTML/CSS/JS/Fields/Data), the primary path. (2) **Drag and drop** of a folder, ZIP, multiple files or widget.io ZIP. (3) A **single text/PDF/DOCX** guide. (4) An optional **one-shot "Import from my SE account"**: paste an overlay token or JWT, `GET /kappa/v2/channels/me`, list overlays, then `GET /kappa/v2/overlays/{channel}/{id}` and extract `variables.html/css/js/fields/fieldData` for each custom widget. Never persist the token ([streamwizard §1](https://github.com/streamwizard/streamwizard/blob/main/docs/streamelements-import-plan.md)). This also migrates AlertBox settings and layout later |
| **Role detection** | By name: `*.html`/`html.txt` → HTML; `*.css`/`css.txt`; `*.js`/`js.txt`/`script.js`; `fields.json`/`fields.txt`/`widget.json`; `data.json`/`data.txt`/`fielddata.json`; `widget.ini` overrides. **By content** when names are ambiguous: a JSON object whose values have `type` keys → Fields; a flat JSON object → Data. HTML/CSS/JS by parser success |
| **Single-doc splitter** | Split on heading lines matching `^(HTML|CSS|JS|JAVASCRIPT|FIELDS?|DATA)\b` or code fences. Normalize PDF artefacts: smart quotes `“”‘’`→`"'` inside code, ligatures, NBSP, soft hyphens, page headers and footers. Validate each part. Show a side-by-side "we detected…" confirmation |
| **Assets** | Relative media in the ZIP → our media store, with references rewritten. `cdn.streamelements.com/uploads/...` → optional "copy locally" (dedupe by UUID), otherwise keep remote |
| **Normalize** | Protocol-relative `//host` → `https://host`, because our page is `http:` (SE2SB does the same). Strip BOM |
| **Compat lint report** (before first run) | Listeners used; `SE_API` methods; SE REST endpoints (handled / shimmed / unsupported); `apiToken` usage; external hosts; `{x}` tokens not matching any field; `localStorage`/`indexedDB`; `eval`; button-event shape; Twitch-only assumptions (`provider==="twitch"`, hard-coded Twitch badge URLs, Helix calls). Produces a **score** and "will degrade" notes. AI-assisted fix suggestions are opt-in only and need explicit consent (license) |
| **Persist** | `widget` record: parts, fields schema (raw + normalized), data, assets, source/provenance, license note (free text), lint report, compat mode flags (A6.7) |

### A6.3 Isolation, serving and network

| Decision | Recommendation | Why |
|---|---|---|
| Origin | **One real origin per widget instance**: `http://w-<hash>.localhost:<port>`. Chromium resolves `*.localhost` to loopback and treats it as a potentially-trustworthy origin (**verify in OBS's CEF**). Fallback: a separate port per widget on `127.0.0.1` | An opaque-origin iframe (`sandbox` without `allow-same-origin`, as proposed in designer B.10) makes `localStorage` throw and turns CORS `Origin: null`. That breaks widgets that work on SE. A distinct origin keeps isolation from our app origin and from other widgets |
| Sandbox flags | `allow-scripts allow-same-origin allow-forms`. No top-navigation, popups or downloads | `allow-same-origin` is safe here **only because** the origin differs from the parent's |
| Tokens | The iframe URL carries a **capability** scoped to "read this widget's files and assets". No API access. All dynamic data flows via `postMessage` from the parent overlay runtime, which owns the overlay token. `channel.apiToken` is always `""` | Third-party paid code must not reach our API |
| Network | Default **"SE-equivalent: allowed"**, with the host list shown in the lint report. Per-widget toggles: offline (CSP `connect-src 'none'`, scripts only from cached copies), allowlist, or "cache CDN assets locally" (fetch once, rewrite) | Matches SE behaviour. Most widgets need fonts and CDNs |
| CSP | Per widget: `frame-ancestors` = our overlay origins. `script-src` = self + allowlisted hosts + `'unsafe-inline' 'unsafe-eval'` (widgets need them) | |
| Audio | The browser source must enable "Control audio via OBS" (designer B.14). In the editor preview, autoplay needs a user gesture or a webview autoplay policy flag | |
| OBS URL | Full overlay `/o/<id>` or **single-widget** `/o/<id>/w/<layerId>`, sized to the widget. Etsy guides tell buyers to add one browser source per widget | |

### A6.4 Bootstrap sequence inside the iframe

1. **prelude.js:**
   - Bridge, console capture, and `window.onerror`/`unhandledrejection` → our dev panel.
   - `localStorage`/`sessionStorage` shims backed by per-widget KV. These are only needed in fallback mode.
   - fetch/XHR interceptor for SE hosts (A6.12).
   - jQuery 3.7.x.
   - `window.SE_API`.
2. Receive `init{fieldData, widgetLoadDetail, compatFlags}` from the parent.
3. Substitute fields into HTML, CSS and JS (A6.5).
4. Parse HTML:
   - Append `<link>`/`<style>` from head to head.
   - Append body nodes except scripts.
   - Then load `<script>` elements **sequentially**: await `onload`/`onerror` for `src` scripts and run inline ones in order.
5. Inject the CSS tab as `<style>`. The JS tab runs as a classic script, so top-level `let`/`const` behave as on SE.
6. Auto-inject `<link>` for each `googleFont` field value (a harmless fallback in case SE does this implicitly, which is unverified).
7. After `DOMContentLoaded` and all scripts: dispatch `onWidgetLoad`. Only then start delivering `onEventReceived`/`onSessionUpdate`.
8. On field edit in the editor: full reload (SE behaviour), unless `setField(..., false)`.

### A6.5 Substitution engine (spec)

- Build a value map from `fields[*].value` defaults, overlaid by `data`. Reserved names (`widgetDuration`, …) are included.
- Pass 1 replaces `{{name}}`. Pass 2 replaces `{name}`, **only for declared field keys**. This avoids corrupting CSS blocks and JS object literals and matches emulator behaviour.
- Insert values raw: strings as-is, numbers and booleans via `String()`, arrays comma-joined. The linter flags text values containing quotes or backticks when the token sits inside a JS string literal.
- No whitespace tolerance inside braces by default. A compat flag can turn it on if field data shows it is needed.
- AlertBox-template imports use a separate variable map (A1.2) bound to our alert context.

### A6.6 `onWidgetLoad` assembly from our state

| SE field | Our source |
|---|---|
| `channel.username` | The primary account login (configurable). Never spoofed |
| `channel.id` | A stable synthetic 24-hex id per profile. It is also the key the REST shim recognises |
| `channel.providerId` / `avatar` | Primary Twitch user id and profile image. For YouTube- or Kick-only users, that platform's ids |
| `channel.apiToken` | `""` |
| `currency` | Profile display currency |
| `fieldData` | Defaults merged with data |
| `recents` | Last 25 events from our event store, SE-shaped `{name, amount, tier, createdAt, type}` |
| `session.data` | **Session aggregator**: computes all ~105 keys from our event store (latest, session/week/month/total, P-top-donation/donator, counts, `-recent` arrays of 25), goals from our Goals feature, totals from platform APIs (Helix followers/subs, YouTube subscriber count, Kick `active_gifted_subscribers_count`…). Unknown families are zero-filled, never missing |
| `session.settings` | `{autoReset:true, calendar:<pref>, resetOnStart:<pref>}` |
| `overlay` | `{isEditorMode, muted}` |
| Multi-platform policy | Per-widget **"Session data source: Combined (default) / Twitch / YouTube / Kick"**. Combined sums counts; `*-latest` = most recent across platforms |

The SE session concept maps to our stream session: start on go-live, reset 15 min after offline (SE default), with a manual reset in the dock.

### A6.7 Event mapping: unified events → SE listeners

**Delivery** goes through the overlay runtime. Alert-type events pass through the widget's bound **alert lane** (A6.11). Chat and control events bypass the lane.

| Our unified event | SE listener | Payload mapping (key fields) | Session keys touched |
|---|---|---|---|
| Chat message (any platform) | `message` | A6.8 | — |
| Message deleted | `delete-message` | `{msgId}` | — |
| User timed out / banned / messages purged | `delete-messages` | `{userId}` (platform-prefixed per A6.8) | — |
| Chat cleared (Twitch `channel.chat.clear`) | `delete-message` × N | The runtime tracks msgIds sent to each widget and emits one per id. SE has no equivalent (unverified) | — |
| Twitch follow (`channel.follow`) | `follower-latest` | `{name: login, displayName, providerId, avatar, _id, sessionTop, type:"follower", originalEventName}` | follower-* |
| Kick follow | `follower-latest` | same | follower-* |
| YouTube new subscriber (polled) | `subscriber-latest` | `{name}` (SE's YouTube shape) | subscriber-* (YT) |
| Twitch sub / resub (`channel.chat.notification` sub/resub, `channel.subscription.message`) | `subscriber-latest` | `amount=cumulative months`, `tier` `"1000"|"2000"|"3000"|"prime"` (`is_prime`), `message` (resub text), `gifted:false` | subscriber-*, -new/-resub-* |
| Twitch directed gift (`sub_gift` notice) | `subscriber-latest` | `name=recipient, sender=gifter, gifted:true, tier` | subscriber-gifted-* |
| Twitch community gift (`community_sub_gift` notice / `channel.subscription.gift`) | `subscriber-latest` once with `bulkGifted:true, amount=total, sender=gifter, name=gifter`, then per recipient `gifted:true, isCommunityGift:true` (linked by `community_gift_id`) | Anonymous: `sender="AnAnonymousGifter"` (unverified SE convention) | subscriber-gifted-*, alltime-gifter |
| Kick sub new / renewal / gifts | `subscriber-latest` | `tier:"1000"` (Kick has no tiers), gifts like Twitch (bulk + per giftee from the webhook giftee list) | subscriber-* |
| Twitch cheer (`channel.cheer`) | `cheer-latest` | `{name, amount: bits, message}`. Anonymous → name "Anonymous" | cheer-* |
| Kick KICKs (`kicks.gifted`) | `kicks-latest` | `{name, amount, message}` | kicks-* |
| Raid (Twitch `channel.raid`) | `raid-latest` | `{name: from login, amount: viewers}` | raid-* |
| Tip (Ko-fi, Throne, Fourthwall, BMC, Streamlabs, SE via Astro…) | `tip-latest` | `{name, amount (converted to display currency), message}` + `ext.originalCurrency/amount` | tip-* |
| YouTube Super Chat / Super Sticker | `superchat-latest` | `{name, amount (converted), message (sticker caption), displayName, avatar}` + ext tier/colours | superchat-* |
| YouTube membership new / milestone | `sponsor-latest` | `amount=months`, `message` (milestone), `tier` = level name in ext | sponsor-* |
| YouTube gifted memberships purchase / received | `sponsor-latest` | bulk + per-recipient, same flags as subs | sponsor-gifted-* |
| YouTube Jewels gift | `event` (`type:"jewels"`, provider youtube) + ext | Listener name unverified. SE added "Jewels" 2026-09-17 | (none documented) |
| Twitch channel-point redemption | `event` | `{type:"channelPointsRedemption", provider:"twitch", data:{username, displayName, redemption, message, amount, quantity, avatar}}` (observed shape) | `channel-points-latest` |
| Hype Train begin/progress/end | `event` (`type:"hypetrain"`) (unverified) | session keys are authoritative | hypetrain-* |
| Charity donation | `charityCampaignDonation-latest` (unverified) | `{name, amount}` | charityCampaignDonation-* |
| Merch/Shop order (Fourthwall/Shopify) | `merch-latest` | `{name, amount, items[{name, quantity}]}` | merch-* |
| Counter changed (our counters) | `bot:counter` | `{counter, value}` | — |
| Widget KV changed | `kvstore:update` | `{data:{key:"customWidget.<k>", value}}` | — |
| Alert lane skip | `event:skip` | `{}` | — |
| Alerts mute toggle | `alertService:toggleSound` | `{muted}` | — |
| Field button clicked | `event:test` | `{listener:"widget-button", field, value}` (observed shape, default). If the lint finds only the documented pattern (`detail.listener==='widget-button'`) and no `event:test`, flag `buttonShape:"documented"` and dispatch `{listener:"widget-button", event:{field, value, listener:"widget-button"}}` | — |
| Editor "Emulate" | the real listener with `isTest:true` + `detail.emulated:true` | per Tixyel typings | Not persisted |
| Streamer.live action "Send event to widget" | custom listener name | Mirrors SE's 2026 `overlay.broadcast` idea: lets our actions drive purchased widgets | — |
| Other platforms (TikTok gifts, Rumble rants…) | Opt-in per widget: monetary → `tip-latest`, follows → `follower-latest` | Off by default to avoid surprising alerts | — |

**Common fields** on every `-latest` payload: `_id` (our event id), `activityId`, `sessionTop` (computed), `type`, `originalEventName`, `displayName`, `providerId`, `avatar`. Add an extension object `slive:{platform, eventId, raw?}`.

### A6.8 Multichat → SE `message` payload

**Goal:** every platform's message looks like a well-formed **Twitch-shaped** SE message, so Twitch-designed widgets render it. Native per-platform extras are included, so widgets aware of SE's YouTube shape also work.

| SE field | Twitch (EventSub `channel.chat.message`) | YouTube (streamList `liveChatMessage`) | Kick (`chat.message.sent`) |
|---|---|---|---|
| `service` | `"twitch"` | `"youtube"` | `"kick"` |
| `data.msgId` / `tags.id` | `message_id` | message `id` | message id |
| `data.userId` / `tags.user-id` | `chatter_user_id` (raw) | `youtube:<authorChannelId>` | `kick:<user_id>` (prefix avoids numeric collisions with Twitch ids) |
| `data.nick` | `chatter_user_login` | normalized display name (lowercase, no spaces) | `username` (slug) |
| `data.displayName` / `tags.display-name` | `chatter_user_name` | `authorDetails.displayName` | `username` |
| `data.displayColor` / `tags.color` | `color`. If empty: `""` (SE behaviour) or optional deterministic colour | none: `null` (SE) **or** per-user deterministic colour (default on, toggle) | `identity.color` |
| `data.text` | `message.text` | `displayMessage` with emoji shortcodes kept | text with `[emote:ID:NAME]` → `NAME` (**unverified token format**) |
| `data.emotes[]` | From `fragments` of type emote → `{type:"twitch", id, name, gif:(format includes animated), urls:{1,2,4} (v2 CDN), start, end}` (UTF-16 indexes as JS expects) + our resolved BTTV/FFZ/7TV `{type, name, id, urls, gif}` with start/end added | Custom/member emoji → `{type:"youtube", name, id, urls, start, end}` | `{type:"kick", id, name, urls:{1,2,4}:files.kick.com/emotes/ID/fullsize (unverified), start, end}` + 7TV-for-Kick |
| `data.badges[]` | `badges[{set_id,id,info}]` → `{type:set_id, version:id, url (Helix badge 4x), description}` | **Synthesized**: owner→`broadcaster`, moderator→`moderator`, member→`subscriber` (url = channel member badge if known, else our bundled icon; version = months if known), verified→`partner`/`verified` | `identity.badges` (broadcaster, moderator, vip, og, founder, subscriber(months), sub_gifter, verified, staff) → Twitch-like types (`og`/`founder` keep their names) |
| `data.tags` (strings) | **Synthesized IRC tags**: `badges` "set/id,…", `badge-info` "subscriber/<months>", `mod`, `subscriber`, `vip` (if badge), `turbo` "0", `user-type`, `room-id`=`broadcaster_user_id`, `user-id`, `display-name`, `color`, `emotes` "id:s-e/…", `id`, `tmi-sent-ts`, `flags` "", `first-msg`/`returning-chatter` (if known), `msg-id` ("highlighted-message", "user-intro" from `message_type`), `reply-parent-msg-id/-body/-display-name/-user-login/-user-id` from `reply` | Synthesized the same way: `room-id` = channel id, so **owner messages satisfy the common `room-id === user-id` broadcaster test**, `mod`/`subscriber` from flags. SE itself sends `tags:[]`, so a compat flag can mimic that | Synthesized the same way: `room-id` = channel user id |
| `data.channel` | broadcaster login | channel id (SE) | channel slug |
| `data.isAction` | `/me`: EventSub has no explicit flag. Detect ACTION in text (unverified) | false | false |
| `data.time` | receive time (ms) | `publishedAt` | `created_at` |
| `data.avatar` | added as an extension (we fetch profile images anyway) | `authorDetails.profileImageUrl` (SE field) | `profile_picture` |
| `renderedText` | HTML-escaped text with `<img class="emote" src srcset title>` per emote, cheermotes rendered | same | same |
| YouTube native extras | — | `kind`, `etag`, `id`, `snippet{…}`, `authorDetails{…}` (SE's documented shape) | — |
| Extension | `data.slive = {platform, channelId, sourceBroadcaster (shared chat), isFirstMessage, raw?}`. A namespaced object, so it cannot collide with future SE fields | same | same |

**Per-widget platform policy:**
- **Platforms shown:** all, or a subset. A filter profile from our multichat filters can apply.
- **Service presentation:** honest (default) vs "present everything as Twitch". The latter is for widgets that drop lines unless `service==="twitch"` or that branch on the SE provider lookup.
- **Colour fill** for colourless platforms.
- **Badge synthesis** on/off.
- **ID prefixing** on/off.

### A6.9 Deletes and moderation

| Platform event | SE emission |
|---|---|
| Twitch `channel.chat.message_delete` | `delete-message {msgId}` |
| Twitch `channel.chat.clear_user_messages` (ban/timeout) | `delete-messages {userId}` |
| Twitch `channel.chat.clear` | per-msgId `delete-message` (tracked) |
| YouTube `userBannedEvent` | `delete-messages {userId:"youtube:<id>"}`. Deletion events were removed from YouTube docs on 2026-06-23 ([platform-apis](./platform-apis.md)), so per-message delete may be unavailable |
| Kick `moderation.banned` | `delete-messages`. Per-message delete webhook (unverified) |
| Our own moderation (dock hide / AI filter) | `delete-message` (hiding a line in our dock can retract it from widgets) |

### A6.10 `SE_API` emulation

| Method | Backend | Notes |
|---|---|---|
| `store.set/get` | Our KV store, namespace `se:<profile>`, **global across all SE widgets in a profile** (SE semantics). Keys are validated alphanumeric. `set` broadcasts `kvstore:update` to every SE widget in every connected overlay | Also exposed to our actions and variables ("Get/Set SE widget store"), so a Streamer.live action can drive a widget's state |
| `counters.get` | Our counters/globals | Returns a superset `{id, counter, count, value}` to satisfy both documented shapes. Counter changes emit `bot:counter` |
| `sanitize({message})` | Our banned-word / profanity filter | `{skip, result:{message}}` |
| `cheerFilter(msg)` | Strip cheermotes using Helix Get Cheermotes prefixes | |
| `getOverlayStatus()` | `{isEditorMode, muted}` | |
| `setField(k, v, reload)` | Editor: updates the unsaved form value, optionally reloads. Live: no-op + console note | |
| `resumeQueue()` | Releases this widget's lane hold (A6.11) | |
| `sendMessage`, `responses` | Stubs plus a console warning | undocumented |

### A6.11 Queue integration (`widgetDuration`, `resumeQueue`)

- Each SE widget instance binds to an **alert lane** (default: the overlay's `main` lane from designer B.9).
- When the lane releases an alert-type job, every widget on the lane receives the SE event. Widgets with `widgetDuration > 0` register a **hold** on the lane.
- The lane waits until **all holds** are released, by `resumeQueue()` or by timeout = `widgetDuration`, plus our native AlertBox `alert.finished` acks.
- Non-queued listeners (A1.11 skippable list) are delivered immediately.
- Lane pause, skip and mute come from the dock or API and emit `event:skip` and `alertService:toggleSound`.
- If no runtime is connected to a lane, the existing B.9 hold/auto-skip policy applies.

### A6.12 SE REST shim (in-iframe fetch/XHR interceptor → parent → core)

| Request | Handling |
|---|---|
| `GET api.streamelements.com/kappa/v2/channels/{ourSyntheticId or username}` | Synthetic profile `{_id, username, displayName, avatar, provider: <primary or "twitch">, providerId}` |
| `GET …/kappa/v2/speech?voice=X&text=Y` | Option A (default): **our local TTS** (Speaker.bot replacement), with a voice map for SE voice names (Brian, Amy…) and audio bytes returned. Option B: pass through to SE's public endpoint (SE's terms, availability risk) |
| `POST …/kappa/v2/bot/{channel}/say` | Map to our "send chat message" (primary platform) **behind a per-widget permission prompt** |
| `GET …/kappa/v2/points/{channel}/{user}` etc. | Our loyalty points if that feature exists, else 404 + lint note |
| `GET …/kappa/v2/sessions/{channel}` | The session aggregator |
| Anything else on `api.streamelements.com` / `realtime.streamelements.com` | Pass through (no token, so it will usually fail) + lint "unsupported" |

This is a compatibility aid inside the widget's own JS, not a security boundary.

### A6.13 Fields JSON → our schema-driven form

| SE | Our field (designer B.10 union) | Notes |
|---|---|---|
| `text` | `text` | Long values auto-grow into a textarea UI. Saved as string |
| `number` / `slider` | `number` / `slider` (min, max, step) | Keep numeric type in `fieldData` |
| `checkbox` | `checkbox` | |
| `colorpicker` | `colorpicker` with alpha, **preserving the input format** (hex vs rgba) | Values land verbatim in CSS |
| `dropdown` | `dropdown`. Options from `{key: label}` **in JSON order** (JS reorders integer-like keys, so do not round-trip through a JS object) | value = key |
| `image-input` / `video-input` / `sound-input` | `image`/`video`/`sound` asset pickers from our local media library. `multiple` → array | URLs absolute to our asset origin, readable from widget origins |
| `googleFont` | `fontpicker` (Google list + local fonts) with an optional "cache locally" | |
| `button` | `button` → dispatches the button event (A6.7) | |
| `hidden` | not rendered. `widgetName`/`widgetAuthor` → layer name/credit, `widgetDuration` → lane hold, `widgetVersion`/`widgetUpdateUrl` → shown in "About" | |
| other HTML5 types | `text` with input-type hint (`password` masked) | SE fallback behaviour |
| `group` | collapsible group in JSON order of first appearance | |

The same form component renders native widgets. Imported SE widgets therefore look native in the designer's properties panel. The raw JSON stays editable in a "Code" tab (Monaco), like SE's OPEN EDITOR.

### A6.14 Test events and editor UX

- **Test toolbar** per SE widget: Message (Twitch / YouTube / Kick variants: emotes, badges, `/me`, reply, long text, RTL, zalgo), Delete message, Purge user, Follow, Sub (T1/T2/T3/Prime, resub with message), Gift (directed / community ×5 with recipients), Cheer, Raid, Tip, Super Chat, Member, KICKs, Redemption, Counter, KV update, Skip, Mute.
- Each test dispatches with `isTest:true`/`emulated:true` and never touches session data unless "Include in session" is ticked.
- **Replay last N real events** from our event store into the widget.
- **Console/errors panel** per widget. **Network panel**: hosts contacted, blocked requests.
- **Lint report** with a compat score (A6.2).

### A6.15 Compatibility test plan

| Item | Plan |
|---|---|
| **Fixture corpus (committed)** | Only permissively licensed sources: `jhoooooo/chatbox` (MIT), `swolekat/se-widgets` (MIT, ~29 chat units), `reboot0-de/se-tools` (MIT), `Tixyel/widgets` (Apache-2.0), `liquidnya/pronouns-chat` (MIT, except files with their own headers: check), **plus ~15 clean-room fixtures we author**, each exercising one API corner: every listener, both button shapes, `{x}` in JS, script-order dependency, GSAP via CDN, `localStorage`, `resumeQueue` with `widgetDuration`, session keys, `onSessionUpdate`, `store` broadcast, REST provider lookup, speech, rgba colorpicker, `multiple` media |
| **External corpus (not committed)** | A dev script downloads unlicensed repos (StreamElements/widgets, FabioZumbi12, brofar, c4ldas, j3channel…) into a local cache for testing only. **No redistribution** |
| **Private corpus** | The owner's purchased Etsy widgets, tested locally. Results are recorded as anonymised metrics only (no code in the repo) |
| **Harness** | Playwright/Chromium (and periodically OBS's CEF version) loads each widget through the real runtime. Scripted scenario: load → 10 Twitch messages (emotes, BTTV, badges, action, reply) → 5 YouTube → 5 Kick → delete-message → delete-messages → follow/sub/gift-bomb/cheer/raid/tip/superchat/member/KICKs → button → KV update |
| **Assertions** | No uncaught errors. Message text present in DOM. Emote `<img>` count matches. Deleted nodes removed. Alert-type events produce DOM changes. Queue hold released within `widgetDuration`. Screenshot diffs against goldens (per platform). Lint score vs actual-outcome calibration |
| **Metrics** | % units: "fully works" / "works, degraded on YT/Kick" / "broken" with reason codes. Target ≥85% fully working for chat boxes in the private Etsy set |
| **Regression** | Run on every runtime change. Re-run nightly against the external corpus |

### A6.16 Known gaps (cannot or will not emulate)

- **SE cloud services:** loyalty points and store, SE bot commands and timers, contests/giveaways, media requests, SE Pay tips (unless the user still connects SE via Astro WS, which [platform-apis](./platform-apis.md) already covers), SE merch, arbitrary JWT/apiToken REST calls.
- **Exact SE semantics:** the queue timing nuances, `sessionTop` rules, weekly/monthly calendar vs rolling windows (setting), `_id`/`activityId` formats (synthetic).
- **SE native widgets** (AlertBox, labels, goals, event list, KappaGen, chat box) are not custom code. We provide native equivalents. An importer can map their settings from overlay JSON later (Lumia/streamwizard pattern).
- **Platform realities:** YouTube has no name colours or Twitch-style badge images. Kick and YouTube deletes are limited. Twitch "host" no longer exists.
- **Widgets with Twitch-only logic:** Helix calls with their own client IDs, hard-coded Twitch badge URLs by `type/version`, decapi lookups by Twitch login. These degrade on YouTube and Kick lines.
- **Obfuscated widgets** (widget.io offers obfuscation) run, but lint is weaker.
- **Seller channel-locks:** honoured, not bypassed.

### A6.17 Effort sketch

| Phase | Scope | Size |
|---|---|---|
| 1 | Importer (paste + files/zip + ini), substitution, iframe origin and loader, jQuery, `onWidgetLoad` minimal, Twitch `message`/deletes, button events, fields → form, test toolbar | M |
| 2 | Full Twitch event mapping + session aggregator + `onSessionUpdate` + `SE_API` (store/counters/queue) + lane holds | M–L |
| 3 | YouTube/Kick message mapping with tag/badge synthesis, per-widget platform policy | M |
| 4 | REST shim (channels, speech → our TTS, bot say), lint report + score, single-doc/PDF splitter, asset localisation | M |
| 5 | Optional SE-account import (overlay layout + AlertBox → native), Streamlabs layer | L |

---

## A7. The StreamElements overlay editor: mental model

**How it works:**
- An **overlay** has a **fixed-resolution canvas** (e.g. 1920×1080) chosen at creation, and becomes **one browser source** ([getting-started](https://docs.streamelements.com/overlays/getting-started), [broadcast-software](https://docs.streamelements.com/overlays/broadcast-software)).
- Widgets are absolutely positioned layers (`top/left/width/height/z-index/opacity`), with flat groups ([overlay.update](https://docs.streamelements.com/websockets/topics/channel-overlay-update)).
- **"+"** opens a categorised widget library: Alerts (AlertBox, KappaGen), Static/Custom (Custom widget, text, image, video), Stream Tools (chat), labels, goals…
- Selecting a widget opens a **left panel** with common settings plus type-specific settings. Custom widgets show their **fields form** and an **OPEN EDITOR** code window with 5 tabs.
- A **layers dropdown** supports rename (double-click), lock and visibility.
- The top bar has **Emulate** (test alerts), **mute**, **save**, and a **link icon to copy the overlay URL** (which contains a secret token).
- Keyboard shortcuts: Delete, Shift+D duplicate, arrows 1px / Shift 10px, Esc, Ctrl+Z/Y, Alt+X reset zoom, Alt+F fit, Shift+G/U group/ungroup ([shortcuts](https://docs.streamelements.com/overlays/overlay-editor-shortcuts)).
- A separate **Widget Data** dashboard edits session data, labels and goals ([widget-data](https://docs.streamelements.com/overlays/widget-data)).
- Variations and the alert queue live inside the AlertBox ([alertbox-variations](https://docs.streamelements.com/overlays/alertbox-variations)).

| SE editor trait | Copy? | Our improvement |
|---|---|---|
| Fixed canvas + absolute layers | **Copy** (it is the model the owner has in mind) | Add vertical/aspect *variants* of one overlay, anchors for responsive re-layout, snapping guides, align/distribute, real nested groups |
| "+" categorised library | **Copy** | Searchable. Imported SE widgets and installed packages appear alongside native ones |
| Fields-driven settings panel for custom widgets | **Copy** (fields → form) | The same form system for native widgets. Field groups, `visibleIf`, live preview without reload where possible |
| OPEN EDITOR with 5 code tabs | **Copy** | Monaco with lint, compat report, per-widget console and network panes, format-on-paste, smart-quote repair |
| Emulate test events | **Copy** | Per-platform tests, gift bombs, replay of real events, recorded scenarios |
| Copy overlay URL | **Copy** | Plus **single-widget URLs**, per-source tokens, and a "drag to OBS" / obs-websocket "add to scene" button |
| Editing the live overlay directly (saves go live) | Improve | **Draft vs Published**: edit while live without the audience seeing half-done changes, plus version history and undo across sessions |
| Widget Data dashboard for session fixes | **Copy** | Session editor in the dock, with an audit trail |
| One AlertBox shared across scenes via a dedicated overlay | Improve | Alert lanes are first-class and independent of overlays (designer B.9) |
| Cloud dependency, token in URL | Improve | Local-only, LAN optional, per-overlay revocable tokens |
| `isEditorMode` flag to widgets | **Copy** | Widgets can show placeholders in the editor. We pass it for SE and native widgets |

---

# Part B — Node-based action editor

## B1. Node-graph libraries for React (2026-10-18, npm registry)

| Library | Version (date) | License | Rendering | Fit | Notes |
|---|---|---|---|---|---|
| **React Flow `@xyflow/react`** | **12.12.0 (2026-09-24)** | MIT | DOM/SVG; nodes are React components | **Best** | 38.7k★, very active (perf work in 12.11.2, `EdgeToolbar` 12.9, a11y 12.7) ([what's new](https://reactflow.dev/whats-new)). Sub-flows via `parentId` (good for regions). `isValidConnection` for structural constraints. Perf guidance: memoize nodes, never read the whole `nodes` array in node components, collapse large trees, light node CSS ([perf](https://reactflow.dev/learn/advanced-use/performance)). No published node-count ceiling. 200+ memoized nodes with `onlyRenderVisibleElements` are expected to be fine (**benchmark in spike**). Pro subscription optional |
| **Rete.js v2** | `rete` 2.0.6 (2025-06), `rete-react-plugin` 2.1.2 (2026-07), `rete-area-plugin` 2.3.2 | MIT | Framework-agnostic; React/Vue/Angular/Svelte renderers | Good alternative | 12.3k★, active. Has `rete-engine` with **ControlFlowEngine** (UE-Blueprint-like) and **DataflowEngine** ([engine](https://retejs.org/docs/concepts/engine)). Scopes plugin for nesting, auto-arrange via ELK. More plumbing, smaller ecosystem. We don't need its engine (ours is Rust) |
| litegraph.js (original) | 0.7.18 (2024-01) | MIT | Canvas2D | Avoid | 8.2k★, unmaintained since 2024-08 |
| ComfyUI litegraph fork | `@comfyorg/litegraph` 0.17.2, **deprecated**; repo **archived** | MIT → now inside **GPL-3.0** `ComfyUI_frontend` | Canvas2D | **Avoid** | Maintained code now lives in a GPL-3.0 repo ([archived repo](https://github.com/Comfy-Org/litegraph.js), [frontend](https://github.com/Comfy-Org/ComfyUI_frontend)). Canvas makes React form widgets in nodes painful |
| Flume | 1.2.0 (2025-11) | MIT | React | Narrow | Dataflow "logic graph" only, no exec flow |
| BaklavaJS | 2.8.1 (2025-11) | MIT | **Vue** | No | wrong framework |
| reaflow | 5.4.1 (2025-04) | Apache-2.0 | React + ELK | Diagram-ish | Auto-layout-first, less editor-oriented |
| `@projectstorm/react-diagrams` | 7.0.4 (2024-02) | MIT | React | No | stale |
| AntV X6 | 3.1.8 (2026-08) | MIT | SVG/HTML, framework-agnostic | Possible | Chinese-first docs/ecosystem |
| JointJS core / JointJS+ | 4.3.3 (2026-09) | MPL-2.0 / commercial | SVG | No | Best features are commercial |
| GoJS | 4.0.5 (2026-10) | Commercial | Canvas | No | license cost |
| Drawflow | 0.0.60 (2024-09) | MIT | vanilla | No | stale |
| **Layout: elkjs** | **0.12.0 (2026-07-17)** | **EPL-2.0 OR GPL-3.0** (choose EPL-2.0, weak file-level copyleft; fine as an unmodified dependency, include the notice) | worker-capable | **Recommended** | Layered algorithm with **ports** (`portConstraints: FIXED_ORDER`) and **compound/hierarchical** graphs (`hierarchyHandling: INCLUDE_CHILDREN`). Exactly what exec ports + nested regions need ([repo](https://github.com/kieler/elkjs)) |
| Layout: `@dagrejs/dagre` | 3.1.1 (2026-08-08) | MIT | — | Fallback | Simple, fast. Weak on ports/compound nodes. (Old `dagre` 0.8.5 is from 2019) |

**Recommendation:** `@xyflow/react` 12.x + elkjs (in a Web Worker).
- React Flow lets each node body reuse our **existing sub-action form components** (the same JSON-Schema-driven forms used in the list view).
- It has the largest community, MIT license, and active maintenance.

## B2. Prior art

| Tool | Model | Takeaway |
|---|---|---|
| **Unreal Blueprints** | **Exec pins** (white) order impure nodes. **Data pins** (typed, coloured) carry values. **Pure** nodes have no exec pins and re-evaluate on demand. Branch has True/False exec outs ([nodes](https://dev.epicgames.com/documentation/en-us/unreal-engine/nodes-in-unreal-engine)) | Adopt the exec/data split and pure-node concept. Blueprints allow arbitrary exec merges and loops via macros, which we restrict |
| **Node-RED** 5.0.8 (2026-10) | Messages (`msg.payload`) flow along wires. Nodes have ≤1 input and N outputs (fan-out allowed). Subflows. node/flow/global context ([concepts](https://nodered.org/docs/user-guide/concepts)). Twitch nodes are niche (e.g. `@xureilab/node-red-twitch-eventsub` 0.1.1, 2024) | Dataflow + fan-out is flexible but **not tree-convertible**. Borrow subflows (≈ our Run Action / groups) and context scopes (≈ args/globals) |
| **SSN Event Flow** | Sources → logic → actions. Each wire carries payload **plus a boolean gate**. True/false outputs, AND/OR/NOT merges, state nodes (counter, gate, rate limiter), no loop node, "break cycles intentionally" ([guide](https://socialstream.ninja/actions/event-flow-guide.html)) | Shows streamers will use graphs, and shows the confusion that comes with cycles and floating NOT gates. Our structured constraints avoid this |
| **Warudo / VNyan** (VTuber) | Blueprint-style graphs with streaming triggers ([app-integrations-vtuber.md](./app-integrations-vtuber.md)) | Our users already know this visual idiom |
| **Bloopbot** (cloud Twitch bot), **StreamGraph** (Godot, node-graph stream deck) | Trigger → condition → action flow editors ([AlternativeTo](https://alternativeto.net/software/bloopbot/about/), [itch](https://eroaxe.itch.io/streamgraph)) | Niche, but they validate demand |
| Streamer.bot / Firebot / Mix It Up / Touch Portal / SAMMI / Lumia | **Lists/trees** of sub-actions/effects. No node editor found | No direct competitor has a graph view over a structured action model. A graph view would be a differentiator. No public Streamer.bot request thread was found (owner reports demand) |

## B3. Recommended design: the graph as a bidirectional *view* of the tree

### B3.1 Canonical model (unchanged; see streamerbot-engine-ui §2.2)

```
Action  := { triggers[], root: Block, vars-schema }
Block   := Stmt[]
Stmt    := Op{ id, type, params(JSON, may contain Expr), outputs } 
         | Group{ id, name, random?, weights?, body: Block }
         | If{ id, cond: Expr, then: Block, else: Block }
         | Switch{ id, subject: Expr, cases: [{values[], body: Block}], default: Block }
         | While{ id, cond: Expr, body: Block } | Break | Continue | Stop
         | RunAction{ id, target, inline: bool, args }
         | Parallel{ id, branches: Block[], join: all|any|none }   // only if the engine adopts it
         | Delay{…} | Code{…}
Expr    := Literal | VarRef(arg|global|output(stmtId, name)) | Call(pureFn, Expr[]) | Template(string with refs)
```

### B3.2 Tree → graph mapping

| Tree construct | Graph rendering |
|---|---|
| Trigger(s) | One **Start** node (lists triggers) with one exec-out |
| Block (sequence) | A chain of exec edges `out → in` |
| `Op` | Node with **exec-in** and **exec-out**. **Data-in pins** for params marked portable (default: scalars; objects via inspector). **Data-out pins** for declared outputs |
| `If` | Node with exec-in, a **cond** data pin (or inline condition editor), and exec-outs **True**, **False**, **Then**. True/False regions render inside a **sub-flow frame** (React Flow parent node) or as a free chain that must terminate (no exec-out wired). Continuation always leaves from **Then** |
| `Switch` | exec-outs per case + **Default** + **Then**, each case a region |
| `While` | exec-outs **Body** + **Completed**. The body region renders inside a loop frame. The back-edge is drawn as a decoration, not a real edge |
| `Group` | A collapsible **frame**, or a collapsed "subgraph" node. A random/weighted group shows a dice badge and weights per child |
| `RunAction` | A node whose header links to the target. `inline` = solid exec line, queued = dashed "fire and forget" |
| `Parallel` | **Fork** node with N branch outs + **Join** output (join policy shown) |
| `Break`/`Continue`/`Stop` | Terminal nodes with no exec-out (validated to sit inside `While` for break/continue) |
| `Expr` (pure: math, string format, get arg/global, compare) | **Pure nodes**: data pins only (Blueprint-style). Wiring them into a param pin builds an expression tree. Unwired params stay inline literals/templates |
| `VarRef(output(stmtId, name))` | A **data edge** from the producing node's out-pin to the consuming param pin |

Node and edge positions live in **view metadata** (`action.ui.graph = {positions: {stmtId: [x,y]}, collapsed: [...], viewport}`). They never change semantics, and diffs and API stay clean.

### B3.3 Constraints that keep the graph convertible (enforced live)

1. **Single entry.** Every exec node is reachable from Start. Unreachable nodes are shown as a "parking lot" region and are **not executed**. Saving warns, or converts them to disabled statements.
2. **No exec fan-out and no exec merges.** Each exec-out connects to ≤1 exec-in, and each exec-in has ≤1 incoming edge. The only joins are the implicit **Then/Done/Completed/Join** ports of control nodes. This gives single-entry/single-exit regions, which form a tree. React Flow's `isValidConnection` rejects violations with an explanation ("Use a Parallel block to run things side by side", "Connect after the If's *Then* port").
3. **No gotos or back-edges.** Loops only via `While` (plus Run Action recursion, guarded at runtime).
4. **Regions are closed.** An edge cannot leave an If/Switch/While/Group region except through its continuation port.
5. **Data edges respect execution order.** The producer must **dominate** the consumer in the exec tree, e.g. not from inside a `False` region into code after the If unless both branches produce it. Otherwise the edge is drawn amber "may be undefined", matching Streamer.bot argument semantics, and the tree stores it as a normal `VarRef`.
6. **Pure subgraphs are acyclic** and must terminate in a param pin. They compile to `Expr`.
7. **Parallel:**
   - If the engine keeps Streamer.bot semantics (sequential, with async via queued Run Action), fan-out is disallowed and offered as "convert to queued Run Actions".
   - If we add `Parallel{join}` (recommended: alerts + TTS + OBS moves often want it), Fork/Join is the only legal fan-out.

**Graph → tree algorithm:**
1. Start at Start and walk exec-outs. Each node becomes a statement.
2. Control nodes recurse into each region port until a terminal or a dangling exec-out (end of region), then continue from the continuation port.
3. Pure subgraphs feeding a param are serialized to `Expr`, and data edges become `VarRef`s.

Because constraints are enforced at edit time, the conversion is total and deterministic, and **round-trip (tree→graph→tree) is identity**. This is a property test in CI over every built-in example and imported Streamer.bot action.

### B3.4 Layout

- **Imported or AI-generated actions** (no positions): run ELK `layered` with `elk.direction=DOWN` (exec flows top→bottom like a list, which eases mental mapping), `portConstraints=FIXED_ORDER` (True, False, Then left→right), `hierarchyHandling=INCLUDE_CHILDREN` for regions, and data edges at lower priority (`elk.layered.priority`, or route them as splines after layout).
- **Incremental:** when the list view inserts or reorders statements, keep stored positions for unchanged nodes and lay out only new ones (ELK interactive layering, or pin existing nodes). "Tidy up" re-runs the full layout.
- Run in a **Web Worker** (elkjs supports it). Cache per action.

### B3.5 Plugin sub-actions as nodes (from JSON Schema)

- **Header:** plugin icon and colour, `title`, and a category badge.
- **Exec:** one in, one out. A plugin may declare *structured* branch outputs (e.g. `x-ui.branches: ["Success","Failure"]`). These are rendered like If regions, with an implicit **Then**, and are stored as an `Op` with region children. Plugins cannot create free-form exec edges.
- **Data-in pins:**
  - One per top-level param property. Scalar `string/number/integer/boolean/enum` params are pinnable.
  - `object`/`array` params are edited in the node's inline form or the side inspector (the same JSON-Schema form renderer as the list view), unless `x-ui.port: true`.
  - Pin colour by JSON type. `format` (e.g. `color`, `uri`) gives specialised editors.
  - Required params show a red dot until set or wired.
  - Unwired pins show the inline literal/template editor in the node.
- **Data-out pins:** from the manifest's `outputs` schema (which already "become variables for subsequent sub-actions", per plugin-sdk §B2).
- **Dynamic options** (`options/provide`) work in the node, as in the list view.
- **Versioning:** if a plugin's schema changes, nodes show a migration badge, and orphaned wires render dashed red.

### B3.6 UX for switching views

- **Per-action toggle `List ⇄ Graph`.** Both are views on one document, with **one undo stack**: edits are commands on the canonical tree, such as `insertStmt`, `moveStmt`, `setParam` and `wire(VarRef)`. Selection syncs across views.
- **Default view:** List for actions under ~15 statements, Graph is user preference. "Open in graph" lives on any group.
- **Graph extras:**
  - Mini-map, search, "collapse all regions", and frames coloured by region type.
  - Inline **Explain/convert hints** when a connection is rejected.
  - **Live execution trace:** replay `Raw.SubAction`-style trace events by highlighting nodes and animating edges. This is the single most persuasive debugging feature.
  - Diff view for AI-proposed changes, rendered as a graph (see mcp-and-ai-providers.md §AI diff).
- **Code nodes** show the language badge and open the code editor in a side panel.
- **Accessibility:** a keyboard path to every operation, via React Flow a11y props plus our list view as the accessible equivalent.

### B3.7 Risks

- **Graph sprawl for big actions:** mitigate with collapse and subgraphs.
- **Users expecting free-form wiring like Node-RED:** constraint messaging must teach, not just block.
- **Data-edge dominance checks are subtle:** start permissive (amber warning), tighten later.
- **React Flow performance with heavy React forms in every node:** render forms only for selected or zoomed-in nodes, and show a compact summary otherwise.

---

## Open questions for the owner

1. **Priority:** should SE widget compatibility move up from P6 (QUESTIONS.md) to v1, given the Etsy use case and SE's 2026 instability?
2. **Isolation:** accept a per-widget real origin (`w-<hash>.localhost`) instead of the opaque-sandbox iframe in designer B.10? This is needed for SE parity (localStorage, CORS).
3. **Network default for imported widgets:** "allowed like SE" with a host report (recommended), or an allowlist and offline-first?
4. **Multichat presentation default:** honest `service` per platform (recommended) or "present everything as Twitch"? Also: badge and colour synthesis on by default?
5. **Session data:** combined across platforms (recommended) or primary platform only?
6. **SE-account import:** offer the optional one-shot "Import from my SE account" (needs the user's SE token)? It conflicts with "no SE account" but rescues share-link-only purchases and full overlays.
7. **SE TTS endpoint:** redirect widget TTS calls to our local TTS by default?
8. **Streamlabs layer:** schedule right after SE, or wait for demand?
9. **Seller ecosystem:** publish a native Streamer.live widget package format with license metadata and invite Etsy sellers (possible partner channel)?
10. **Node editor:** add a `Parallel{join}` block to the engine (enables Fork/Join in the graph), or keep Streamer.bot's sequential model? Also: Graph as the default view for new users?

---

## Sources (primary first)

- **SE official docs** (source repo [StreamElements/docs](https://github.com/StreamElements/docs)):
  - [events](https://docs.streamelements.com/overlays/events), [custom-widget & SE_API](https://docs.streamelements.com/overlays/custom-widget), [widget-structure](https://docs.streamelements.com/overlays/widget-structure), [session-data](https://docs.streamelements.com/overlays/session-data)
  - [custom-code-in-alertbox](https://docs.streamelements.com/overlays/custom-code-in-alertbox), [alertbox-variations](https://docs.streamelements.com/overlays/alertbox-variations), [widget-data](https://docs.streamelements.com/overlays/widget-data), [chat-widget](https://docs.streamelements.com/overlays/chat-widget)
  - [editor shortcuts](https://docs.streamelements.com/overlays/overlay-editor-shortcuts), [broadcast-software](https://docs.streamelements.com/overlays/broadcast-software)
  - WS topics [overlay.update](https://docs.streamelements.com/websockets/topics/channel-overlay-update), [overlay.action](https://docs.streamelements.com/websockets/topics/channel-overlay-action), [overlay.broadcast](https://docs.streamelements.com/websockets/topics/channel-overlay-broadcast)
  - Changelogs (KICKs 2026-09-13, YouTube stickers/Jewels 2026-09-17)
- **SE legacy docs:** [StreamElements/widgets docs](https://github.com/StreamElements/widgets/tree/master/docs), [api-docs Widgets.md](https://github.com/StreamElements/api-docs/blob/main/docs/Widgets.md).
- **SE terms:** [streamelements.com/terms](https://streamelements.com/terms) (2026-08-01).
- **SE status 2026:** [GamesBeat shutdown/reversal](https://gamesbeat.com/streamelements-prepares-to-shut-down-website-and-creator-platform/), [Razer acquisition press release](https://www.gamedeveloper.com/press-release/razer-acquires-streamelements-assets-to-expand-content-creator-ecosystem), [GamesBeat](https://gamesbeat.com/razer-acquires-streamelements/).
- **Community typings and emulators:**
  - [Tixyel/widgets](https://github.com/Tixyel/widgets) (Apache-2.0)
  - [streamelements-devkit](https://github.com/LiamDochartaigh/streamelements-devkit)
  - [se-dev-env](https://github.com/Redo7/se-dev-env) (incl. a real `onWidgetLoad` capture)
  - [widget.io](https://github.com/sigma-cw/widget-io)
  - [c4ldas unofficial API](https://github.com/c4ldas/streamelements-api), [c4ldas-seapi](https://github.com/c4ldas/c4ldas-seapi)
- **Prior art:**
  - [1UP Vision](https://docs.1up.vision/custom-widgets), [SSN importer](https://socialstream.ninja/streamelements-importer.html), [Sound Alerts](https://support.soundalerts.com/article/Custom-Widget), [Lumia importer](https://lumiastream.com/services/streamelements)
  - [SE2SB](https://github.com/imAreinieR/SE2SB-Overlay-Migrator), [sigmanuts](https://github.com/sigma-cw/sigmanuts-webview2), [OpenElements](https://github.com/Raptosaur/OpenElements), [streamwizard plan](https://github.com/streamwizard/streamwizard/blob/main/docs/streamelements-import-plan.md)
- **Streamlabs:** [custom widgets guide](https://streamlabs.com/content-hub/post/how-to-get-started-with-streamlabs-custom-widgets), [alert template params](https://streamlabs.com/content-hub/post/message-template-parameters-for-alert-box-widget-on-streamlabs-twitch-and-youtube), [pronouns-chat](https://github.com/liquidnya/pronouns-chat).
- **Etsy** (via search snippets, pages block fetch): [listing 1854755224](https://www.etsy.com/listing/1854755224/customizable-bubble-chat-widget-stream), [listing 1738148698](https://www.etsy.com/listing/1738148698/animated-twitch-chat-widget-interactive).
- **Legal:** [Google v. Oracle](https://www.supremecourt.gov/opinions/20pdf/18-956_d18f.pdf), [SAS v WPL C-406/10](https://curia.europa.eu/juris/liste.jsf?num=C-406/10).
- **Node editors:**
  - [React Flow what's new](https://reactflow.dev/whats-new), [React Flow performance](https://reactflow.dev/learn/advanced-use/performance), [xyflow repo](https://github.com/xyflow/xyflow)
  - [Rete engine](https://retejs.org/docs/concepts/engine), [elkjs](https://github.com/kieler/elkjs), [dagre](https://github.com/dagrejs/dagre)
  - [Comfy-Org litegraph (archived)](https://github.com/Comfy-Org/litegraph.js), [UE nodes](https://dev.epicgames.com/documentation/en-us/unreal-engine/nodes-in-unreal-engine), [Node-RED concepts](https://nodered.org/docs/user-guide/concepts), [SSN Event Flow](https://socialstream.ninja/actions/event-flow-guide.html)
  - npm registry queried 2026-10-18 for all versions listed.
