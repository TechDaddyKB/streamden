# Streamer.bot: Triggers, Platforms and Integrations (research inventory)

Research date: 2026-10-18. Scope: triggers, streaming platforms, third-party integrations, cloud/online routing, and authentication. Sub-actions, variables, C#, servers and UI are covered in a separate document, so they appear here only where needed to describe an integration.

**How this was gathered**

- Docs site: https://docs.streamer.bot/. All 1,102 raw markdown pages listed in https://docs.streamer.bot/llms.txt were fetched, plus the docs source repo https://github.com/Streamerbot/docs at commit `56253da` (last commit 2026-10-14).
- Changelogs: https://streamer.bot/changelogs/vX.Y.Z, covering every version from v0.1.1 to v1.1.0. The docs-site `/changelogs` pages render empty.
- Official machine-readable manifests published by the Streamer.bot website. These are the most authoritative current lists:
  - Triggers: `https://streamer.bot/api/releases/streamer.bot/latest/manifest/triggers` (467 entries, tagged `1.1.0-alpha.17`)
  - Sub-actions: `https://streamer.bot/api/releases/streamer.bot/latest/manifest/subactions` (462 entries)
  - Events: `https://streamer.bot/api/releases/streamer.bot/latest/manifest/events` (513 WebSocket event types, each with a JSON Schema)
  - Release metadata: `https://streamer.bot/api/releases/streamer.bot`
- To answer the cloud-routing and auth questions, I did a light **strings-only** inspection of the public v1.0.7 release zip. Its SHA-256 matched the published hash. I extracted hostnames and URLs only and did no decompilation. Findings from this step are marked **(observed in v1.0.7 binary)**. Several OAuth client IDs and secrets are embedded in the binary; their values are deliberately **not** reproduced here.
- Anything I did not read directly in docs, changelogs, manifests or binary strings is marked **(inferred)**.

---

## 0. Version status (as of 2026-10-18)

| Channel | Version | Date | Source |
|---|---|---|---|
| **Stable** | **v1.0.7**, a hotfix of v1.0.5/1.0.6 | 2026-08-06 | https://streamer.bot/api/releases/streamer.bot (`release_channel: stable`), https://streamer.bot/changelogs/v1.0.7 |
| Beta (previous) | v1.0.5 | 2026-08-05 | https://streamer.bot/changelogs/v1.0.5 |
| **Alpha** (Patreon Bronze+) | **v1.1.0-alpha.x** (alpha.1 on 2026-10-09; the manifest reports `1.1.0-alpha.17`) | 2026-10-09 → | https://streamer.bot/changelogs/v1.1.0 |
| Earlier milestones | v1.0.0 (2025-08-20: WPF UI, Kick, Cloud Backups, Custom Webhooks, Decks 2.0), v1.0.1 (2025-08-28), v1.0.3 (2026-01-12), v1.0.4 (2026-01-16) | | https://streamer.bot/changelogs/v1.0.0 |

Key changes in the last year:

- **v1.0.0** removed Twitch PubSub. Twitch was then EventSub plus IRC.
- **v1.0.5** removed Twitch IRC. All Twitch traffic, including chat, is now EventSub.
- **v1.0.5** removed Trovo because Trovo stopped offering live streaming.
- **v1.0.5** added Throne, StreamLoots, StreamFog, Stream Deck MCP, YouTube Jewels, and native YouTube New Subscriber polling every 10 s. That polling replaced the StreamElements dependency.
- **v1.1.0-alpha** adds:
  - an integrated Points system
  - Whisper.net voice control
  - Elgato Wave Link 3.x, Elgato Studio and Elgato Control Center
  - a Cron trigger
  - SDL2 controller input triggers
  - OBSBOT (manifest only, undocumented)

  Source: https://streamer.bot/changelogs/v1.1.0

Docs coverage lags the app:

- The API reference has 454 trigger pages; the alpha manifest has 467 triggers.
- Many trigger pages list only "common variables" and no event-specific ones.
- Several guide pages (TipeeeStream, TreatStream, T.I.T.S., Wave Link, Camera Hub, DonorDrive, Decks) are marked WIP.

---

## 1. Trigger inventory

### 1.1 Counts

| Category | Docs API reference (pages) | App manifest 1.1.0-alpha.17 | Notes |
|---|---|---|---|
| Twitch | 131 | 131 | Docs still list removed `Ad Mid-Roll` and `PubSub Connected/Disconnected`. The manifest adds `Power Up Redemption`, `Suspicious User Message` and `Shared Chat Raid`. |
| Elgato | 128 | 128 | Camera Hub 68, Wave Link 20, Wave Link 3.x 19, Studio 9, Control Center 9, Stream Deck 3. Wave Link 3, Studio and Control Center (37 total) are new in 1.1.0. |
| Integrations (3rd-party) | 90 | 97 | The manifest adds OBSBOT (5), StreamLoots `Reaction`, and Streamer.bot `Custom Webhook`. |
| Core | 37 | 41 | The manifest adds Controller Axis, Button and Hat (SDL2, 1.1.0) and Voice Control `Log`. |
| YouTube | 24 | 26 | The manifest adds Poll Started and Poll Updated (handled since 1.0.0). |
| Kick | 18 | 18 | |
| OBS Studio | 9 | 9 | |
| Meld Studio | 8 | 8 | |
| Streamlabs Desktop | 7 | 7 | |
| Custom | 2 | 2 | |
| **Total** | **454** | **467** | The website features page also says "Browse all 467 triggers" (https://streamer.bot/features). |

About 46 of the 467 triggers are 1.1.0-alpha-only: Cron, 3 controller triggers, 37 Elgato triggers and 5 OBSBOT triggers. That leaves **about 421 triggers in stable v1.0.7** (inferred from the docs `version:` tags and changelogs).

Every trigger has a stable numeric event ID (the manifest `value`). Examples: Twitch Follow = 101, Twitch Cheer = 102, Kick Chat Message = 35010. These IDs are used across the WebSocket API. The IDs are grouped into numeric ranges by source: Twitch 1xx/4xx, YouTube 40xx, Kick 350xx, OBS 140xx, Elgato 21xxx and so on. That range structure is worth copying.

### 1.2 Trigger configuration model

Source: https://docs.streamer.bot/guide/core/triggers

- An action can have many triggers, and one event can fire many actions (since 0.2.0).
- There are five filter types:
  - **Basic**: no configuration.
  - **Toggle/Checkbox**: for example, sub tiers.
  - **Select**: a dropdown with an `Any` option.
  - **Range**: Min/Max inclusive. "Max only" means an exact match. **Always Run** lets overlapping range triggers both fire. The default is that the most specific trigger wins.
  - **Text**: for example, a variable-name filter.
- Per-trigger context menu: Test Trigger, Edit, Copy/Paste (Base64), Enable/Disable, Refresh, Clear.
- The **Trigger Viewer** shows every trigger-to-action mapping in one place.
- **Testing**: "Test Trigger" fires with sample data. **Simulated Events** (0.2.3+) let you choose data, such as a user and bits amount, and simulate the event application-wide, so every matching trigger runs. **Core > Test** lets you define custom arguments.
- **Custom triggers** are registered from C#: `CPH.RegisterCustomTrigger(label, name, string[] categoryPath)` and then `CPH.TriggerCodeEvent(name, includeArgs)`. They must be re-registered on every boot, typically in `Init()` with precompile enabled. Nested categories are supported.
- Remote triggers can arrive from another instance through the cloud relay (see §4).
- Many WebSocket events have no trigger. Examples: `Twitch.RewardCreated`, `Raw.Action`, `Application.ActionAdded`, `Misc.StreamerbotExiting`. These are visible only to WebSocket subscribers (https://streamer.bot/api/releases/streamer.bot/latest/manifest/events).

### 1.3 Variable naming conventions (all triggers)

Sources: docs `.variables` definitions at https://github.com/Streamerbot/docs/tree/main/streamerbot/3.api/.variables and https://docs.streamer.bot/guide/core/variables

- **General (every action run):** `actionId`, `actionName`, `runningActionId`, `queueName`, `actionQueuedAt`, `triggerId`, `triggerName`, `triggerCategory`, `__source`, `eventSource`, and `isTest` on test runs.
- **Style:** camelCase scalars, `%name%` interpolation in sub-actions, and **dotted namespaces** for structured data. Examples:
  - `poll.choice#.title`
  - `goal.currentAmount`
  - `obs.sceneName`
  - `fw.*` (Fourthwall), `throne.*`, `shopify.*`, `kicks.*`, `reward.*` (Kick), `charity.amount.value`
- **Indexed lists** use a `#` placeholder replaced by 0..n:
  - `input0`, `input1`
  - `inputEscaped#`, `inputUrlEncoded#`
  - `gift.recipientUser#`
  - `tag#`
  - `line#`
  - Some use bracket forms, such as `term.[#]` and `fw.variants[#].id`.
  - A count variable usually accompanies the list: `input.count`, `tagCount`, `poll.choices.count`.
- **Raw text** is always offered in three forms: `rawInput`, `rawInputEscaped` and `rawInputUrlEncoded`.
- **Common variable groups** are injected by platform. See the table below.

| Group | Variables |
|---|---|
| TwitchUser | `user`, `userName`, `userId`, `userType`, `userGroups`, `isSubscribed`, `subscriptionTier`, `monthsSubscribed`, `isModerator`, `isVip`, `userPreviousActive`, `badges`, `badgeCount` |
| TwitchBroadcaster | `broadcastUser`, `broadcastUserName`, `broadcastUserId`, `broadcastIsAffiliate`, `broadcastIsPartner`. Channel info has also been auto-added since 1.0.5. |
| TwitchChat | `msgId`, `message`, `messageStripped`, `messageCheermotesStripped`, `bits`, `role`, `color`, `colorR/G/B`, `emoteCount`, `emotes`, `isHighlight`, `isAction`, `isReply`, `reply.*`, `firstMessage`, `rawInput*`, `input#*`, `inSharedChat`, `fromSharedChat`, `internal` |
| TwitchSharedChatSource | `source.userId`, `source.userLogin`, `source.userName`, `source.isSubscribed`, `source.isModerator`, `source.isVip` |
| Twitch shared-chat flags | Since 1.0.0, most Twitch events carry `isInSharedChat`, `isSharedChatHost` and `isFromSharedChatGuest` (https://streamer.bot/changelogs/v1.0.0). |
| YouTubeUser | `user`, `userName`, `userId`, `userType`, `isSubscribed`, `isModerator`, `userPreviousActive`, `userProfileUrl`, `userIsSponsor` |
| YouTubeBroadcaster | `broadcastUsername`, `broadcastUserId`, `broadcastUserProfileImage`, `broadcastId` |
| YouTubeBroadcast | `broadcast.id`, `broadcast.title`, `broadcast.description`, `broadcast.privacy`, `broadcast.status`, `broadcast.publishedAt`, `broadcast.defaultLanguage`, `broadcast.defaultAudioLanguage`, `broadcast.tags` |
| KickUser | `user`, `userName`, `userId`, `userType`, `userGroups`, `isSubscribed`, `isModerator`, `userPreviousActive` |
| KickBroadcaster | `broadcastUser`, `broadcastUsername`, `broadcastUserId`, `broadcastUserProfileImage` |
| KickChat | `messageId`, `message`, `messageStripped`, `color*`, `emoteCount`, `emotes`, `badgeCount`, `badge`, `rawInput*`, `input.count`, `input#*` |
| WebsocketClient | `wsIdx`, `wsId`, `wsName`, `wsUrl`, `wsScheme`, `wsHost`, `wsPort`, `wsPath`, `wsQuery` |
| CustomWebsocketServer | `wssIdx`, `wssId`, `wssName`, `ip`, `sessionId` |
| VTubeModel | `modelId`, `modelName` |
| MeldStudioEvent | `meldStudio.id`, `meldStudio.name`, `meldStudio.host`, `meldStudioEvent._json`, `meldStudioEvent.event` |
| Patreon | `id`, `user.id`, `user.type`, `user.Attributes.*`, `member.Attributes.*`, `entitledTiers[#].Attributes.*` |

### 1.4 Core triggers (41 in the manifest)

Reference: https://docs.streamer.bot/api/triggers/core

| Sub-category | Trigger(s) | Key variables / parameters |
|---|---|---|
| Core | **Streamer.bot Started**; **Test** (custom args) | `isTest` |
| Commands | **Command Triggered**; **Command Cooldown** | Triggered: `command`, `commandId`, `commandName`, `commandSource`, `commandType`, `msgId`, `rawInput*`, `input#`, `counter`, `userCounter`, regex `match.count`, `match[i]`, named `<groupName>`, and `commandLocation` (1.0.5). Cooldown: `cooldownLeft`, `globalCooldownLeft`, `userCooldownLeft`. Commands are platform-agnostic. Sources: Twitch Message, Twitch Whisper, Twitch Sub/Resub message, YouTube Message and Kick Message. Matching is Basic (Start/Exact/Anywhere) or Regex. Commands also have cooldowns, Allow/Deny permissions by group or user, and options to ignore the bot account or internal messages (https://docs.streamer.bot/guide/core/commands). |
| File/Folder Watcher | **Changed**, **Created**, **Deleted**, **Renamed** | `watcherFolder`, `watcherFilter`, `fullPath`, `fileName`, `changeType`. Changed also gives `fileSize`, `lineCount`, `line#`. Renamed also gives `oldFullPath`. |
| File IO > File Tail | **Changed** (new line appended) | `filePath`, `fileName`, `line` |
| Global Variables | **Created**, **Updated**, **Deleted**, **User Global Variable Updated** (high-volume) | `global.name`, `global.value`/`newValue`/`oldValue`, `global.persisted`, `global.lastWrite`. The user variant adds `global.userId`, `userType`, `userLogin`, `userName`. |
| Groups | **Added**, **Removed**, **Cleared**, **Users Added**, **Users Removed** | `group.id`, `group.name`, `group.bots`, `group.user#.*` |
| Inputs | **Key Press** (global hotkeys, any combination, replacing the pre-1.0 Hot Keys), **Mouse Click**, **Controller Button / Axis / Hat** (SDL2, 1.1.0) | `modifier`, `hasCtrl`, `hasAlt`, `hasShift`, `key`; `mouse.button`, `mouse.clicks`, `mouse.x`, `mouse.y`. The input hook uses SharpHook/uiohook **(observed in v1.0.7 binary)**. |
| MIDI | **MIDI Message** | `deviceName`, `commandCode`, `channel`, `noteName`, `noteNumber`, `velocity(Normal)`, `controllerNumber`, `value(Normal)`, `rawMessage` (https://docs.streamer.bot/guide/core/midi) |
| Processes | **Process Started**, **Process Stopped** | `name`, `executable`, `path`, `fullPath`, `processId`, `parentProcessId` |
| Quotes | **Quote Added**, **Show Quote** | `quote`, `quoteId`, `quoteUser`, `quoteGame`, `quoteTime` |
| Schedule | **Timed Actions** (interval timers with optional random range and minimum chat-lines gate); **Cron** (1.1.0) | `timerId`, `timerName`, `counter`; `expression`, `triggeredAt` (https://docs.streamer.bot/guide/core/timers) |
| System | **Clipboard Changed** (1.0.0), **Toast Activation** (Windows toast click) | `clipboard.text`, `containsText/Image/Audio`, `formats.#`; `toast.id`, `toast.title`, `toast.message` |
| Voice Control | **Command**, **Dictation**, **Log** (manifest) | `spokenText`, `spokenTextConfidence(Percent)`, `altPhraseText#`, `spokenCommand`. Uses Windows Speech Recognition; 1.1.0 adds Whisper.net (https://docs.streamer.bot/guide/core/voice-control). |
| WebSocket > Client | **Opened**, **Closed**, **Message** (user-defined outbound WebSocket clients) | `message` plus the WebsocketClient group |
| WebSocket > Custom Server | **Connection Opened**, **Connection Closed**, **Message** (user-defined local WebSocket servers) | `data` plus the CustomWebsocketServer group |
| Custom | **Custom** (C# `RegisterCustomTrigger`), **Custom Code Event** (`TriggerCodeEvent`) | Arguments passed from C# |

The HTTP Server, UDP Server and main WebSocket Server have **no triggers**. They are inbound action-execution endpoints, such as `DoAction`, which the other research doc covers (https://docs.streamer.bot/api/servers/http, /udp, /websocket). Streamer.bot Remote, Decks and IFTTT also execute actions directly (§4).

### 1.5 Twitch triggers (131)

Reference: https://docs.streamer.bot/api/triggers/twitch. Most Twitch triggers include TwitchBroadcaster and TwitchUser. Since 1.0.5, **all** Twitch triggers are sourced from EventSub WebSocket.

| Category | Triggers | Key event-specific variables |
|---|---|---|
| Chat (8) | Chat Message; Cheer (Range on bits); First Words (resets after N hours); Whispers; Bot Whispers; Watch Streak; Bits Badge Tier; Modiversary (1.0.5) | Chat uses the TwitchChat group. Cheer: `bits`, `messageCheermotesStripped`. First Words: `firstMessage`; first-message detection has been tracked internally since 1.0.5 because EventSub lacks the flag. Watch Streak: `watchStreak`, `channelPointsAwarded`, `systemMessage`. |
| Chat > Mode (12) | Emote Mode On/Off; Follower Mode On/Off/Changed; Slow Mode On/Off/Changed; Subscriber Mode On/Off; Unique Mode On/Off | `emoteMode`, `followerMode`, `followerModeDuration`, `slowMode`, `slowModeWaitTime`, `subscriberOnly`, `uniqueMode` |
| Channel (4) | Follow (fires once per user per run); Stream Online; Stream Offline; Viewer Count Update | Online: `startedAt`, `game`, `gameId`, `tag#`, `tags`. Offline: `endedAt`, `streamId`. `viewerCount`. |
| Subscriptions (8) | Subscription; Resubscription; Gift Subscription; Gift Bomb; Gift Paid Upgrade; Prime Paid Upgrade; Pay It Forward; Sub Counter Rollover | `tier`, `isMultiMonth`, `multiMonthDuration`, `monthStreak`, `cumulative`, `streakShared`. Gifts: `recipientUser`, `recipientUserName`, `recipientId`, `totalSubsGifted`, `fromGiftBomb`, `anonymous`. Bomb: `gifts`, `totalGifts`, `gift.recipientUser#`, `bonusGifts`. Rollover: `subCounter`, `rollover`, `rolloverCount`. |
| Channel Reward (3) | Reward Redemption; Reward Redemption Updated; Automatic Reward Redemption (built-in rewards such as unlock emote, gigantify, message effects) | `redemptionId`, `rewardId`, `rewardName`, `rewardPrompt`, `rewardCost`, `counter`, `userCounter`, `rawInput*`, `skipsQueue`, `requiresUserInput`, `maxPerStream`, `globalCooldown`, `backgroundColor`, `rewardStatus`. Automatic: `rewardType`, `unlockedEmoteName`, `gigantifiedEmoteUrl`. |
| Channel Power-Ups (2) | Power Up Redemption (1.0.5; split out of Automatic Reward Redemption); Custom Power-Up Redemption (1.0.5) | Payload: `type`, `bits`, `counter`, `userCounter`, `text`, `emote`, `messageEffectId`. Custom: `customPowerUp`, `userInput`, `status`. |
| Hype Train (4) | Start, Update, Level Up, End (EventSub v2 since 1.0.3) | `level`, `prevLevel`, `percent`, `percentDecimal`, `expiresAt`, `duration`, `trainType`, `isGoldenKappaTrain`, `isTreasureTrain`, `isSharedTrain`, `top.bits.*`, `top.subscription.*`, `allTimeHighLevel` |
| Raid (4) | Raid (incoming); Raid Start, Raid Send, Raid Cancelled (outgoing) | `viewers`, `raidUser`, `raidUserName`, `raidUserId`, `raidUserProfileImageURL` |
| Polls (5) | Created, Updated, Completed, Terminated, Archived | `poll.Title`, `poll.Id`, `poll.Duration`, `poll.choice#.title/votes/rewardVotes/totalVotes`, `poll.winningIndex`, `poll._json` |
| Predictions (5) | Created, Updated, Locked, Completed, Canceled | `prediction.Title`, `prediction.outcome#.title/users/points/color`, `prediction.winningOutcome*` |
| Channel Goal (3) | Goal Begin, Progress, End | `goal.id`, `goal.type`, `goal.description`, `goal.currentAmount`, `goal.targetAmount` |
| Community Goal (2) | Contribution, Ended | `title`, `userContributed`, `goalAmount`, `contributed`, `percentComplete`, `daysLeft`. These stopped working when PubSub was removed in 1.0.0 (https://streamer.bot/changelogs/v1.0.0). |
| Charity (4) | Started, Progress, Donation, Completed | `charity.amount.value/valueMinor/currency`, `charity.name`, `charity.targetAmount.*` |
| Ads (2 in manifest) | Ad Run; Upcoming Ad. Ad Mid-Roll is in docs but was removed with PubSub. | `adLength`, `adLengthMs`, `adScheduled`; `minutes`, `nextAdAt`, `snoozesLeft` |
| General (4) | Announcement; Present Viewers (periodic list of chatters); Shoutout Received; Stream Update (title/game change) | `announceColor`; `users`, `isLive`; `viewerCount`; `gameUpdate`, `statusUpdate`, `gameName`, `oldGameName`, `gameBoxArt` |
| Moderation (26) | AutoMod Message Held/Updated; Blocked Terms Added/Deleted; Permitted Terms Added/Deleted; Chat Cleared; Chat Message Deleted; Moderator Added/Removed; VIP Added/Removed; Shield Mode Begin/End; Shoutout Created; Suspicious User Update; Suspicious User Message; Unban Request Created/Approved/Denied; User Banned; User Timed Out; User Unbanned; User Untimed Out; Warned User; Warning Acknowledged | `createdById/Username/DisplayName`, `reason`, `duration`, `messageId`, `category`, `level`, `term.[#]`, `fromAutomod`, `lowTrustStatus`, `moderator.*`, `targetMessageId` |
| Shared Chat (18) | Session Begin/Update/End; Shared Announcement, Raid, Sub, Resub, Sub Gift, Community Sub Gift, Gift Paid Upgrade, Prime Paid Upgrade, Pay It Forward, Message Deleted, User Banned/Timed Out/Unbanned/Untimed Out, Shared Modiversary | `host.userId`, `host.userLogin`, `host.isModerator`, plus the TwitchSharedChatSource group |
| Guest Star (4) | Session Begin/End; Guest Update; Settings Update | `sessionId`, `guestUserName`, `slotId`, `state`, `hostVideoEnabled` |
| Pyramid (2) | Pyramid Success; Pyramid Broken (internal chat-pyramid detector; can count shared-chat participation since 1.0.5) | `pyramidWidth`, `pyramidEmote`, `totalPyramids`, `pyramidOwnerUsername` |
| Emotes (4) | BetterTTV Emote Added/Removed; SevenTV Emote Added/Removed | `emote`, `imageUrl` |
| Connections (7) | Broadcaster Authenticated; Broadcaster Chat Connected/Disconnected; Broadcaster EventSub Connected/Disconnected; Bot Chat Connected/Disconnected | none |

### 1.6 YouTube triggers (26)

Reference: https://docs.streamer.bot/api/triggers/youtube. All include YouTubeUser, YouTubeBroadcast and YouTubeBroadcaster.

| Category | Triggers | Key variables |
|---|---|---|
| Chat (5) | Message; First Words; Super Chat; Super Sticker; Jewels Gifted (1.0.5) | `messageId`, `message`, `publishedAt`, `parts`, `emotes` (YouTube, BTTV, 7TV and Twemoji are parsed). Super Chat: `amount`, `microAmount`, `currencyCode`, `tier`. Sticker: `stickerId`, `stickerImageUrl`. Jewels: `gift.jewelsAmount`, `gift.name`, `gift.isCombo`, `gift.comboCount`. |
| Membership (3) | Membership Gift (gifter side); Gift Membership Received (recipient side); Member Milestone | `count`, `tier`, `gifterUser*`; `months`, `levelName`, `message` |
| General (7) | New Sponsor (new member); New Subscriber (polled every 10 s since 1.0.5, no longer via StreamElements; private subscriptions are not visible); Present Viewers (activity-based, every minute); Sponsor-Only Started/Ended; User Banned; User Timed Out (split out in 1.0.5) | `isUpgrade`, `levelName`; `banType`, `banDuration` |
| Broadcast (8) | Broadcast Added/Removed/Started/Ended/Updated; Broadcast Monitoring Started/Ended; Statistics Updated | `likeCount`, `viewCount`, `concurrentViewers`. Multiple broadcasts can be monitored at once. Non-live monitored broadcasts are auto-unmonitored after 10 min to save quota (0.2.5). |
| Polls (3) | Poll Started, Poll Updated, Poll Closed | `poll.question`, `poll.totalVotes`, `poll.option#.text/votes`, `poll.winningOption.*` |

### 1.7 Kick triggers (18)

Reference: https://docs.streamer.bot/api/triggers/kick. All include KickBroadcaster and KickUser.

| Category | Triggers | Key variables |
|---|---|---|
| Chat (2) | Chat Message; First Words (12 h reset default) | KickChat group, `isInternal`, reply data (1.0.1) |
| Channel (4) | Follow (once per user per run); Stream Online; Stream Offline; Viewer Count Update | `startedAt`, `title`, `category.name/id/thumbnail`, `endedAt`, `viewerCount` |
| Subscriptions (4) | Subscription; Resubscription; Gift Subscription; Mass Gift Subscription (Count criteria) | `expiresAt`, `duration`, `monthsSubscribed`; `recipient.userName`, `recipient.#.*`, `count` |
| Channel Reward (1) | Reward Redemption (1.0.2/1.0.3; Kick native rewards, maximum 15) | `reward.id`, `reward.title`, `reward.cost`, `reward.backgroundColor`, `rawInput*` |
| Kicks (1) | Kicks Gifted (Kick's monetary gifts) | `kicks.amount`, `kicks.name`, `kicks.type`, `kicks.tier`, `kicks.pinnedTimeSeconds` |
| Moderation (2) | User Banned; User Timed Out | `createdBy*`, `reason`, `duration` |
| General (2) | Channel Update (title/category change); Present Viewers (activity-based) | `categoryUpdated`, `categoryName`, `oldCategoryName`, `status`, `oldStatus` |
| Emotes (2) | SevenTV Emote Added/Removed | `emote`, `imageUrl` |

### 1.8 Broadcasting-software triggers

| App | Triggers | Variables |
|---|---|---|
| **OBS Studio** (9) | Connected, Disconnected, Scene Changed, Streaming Started/Stopped, Recording Started/Stopped, **Event** (any raw obs-websocket event, filterable), **Vendor Event** (obs-websocket vendor/plugin events) | `obs.id`, `obs.name`, `obs.host`, `obs.studioVersion`, `obs.websocketVersion`, `obs.sceneName`, `obs.oldSceneName`, `obsEvent.*`, `obsVendorEvent.vendor/event` |
| **Streamlabs Desktop** (7) | Connected, Disconnected, Scene Changed, Streaming Started/Stopped, Recording Started/Stopped | `sd.id`, `sd.name`, `sd.host`, `sd.sceneName` |
| **Meld Studio** (8) | Connected, Disconnected, Scene Changed, Streaming Started/Stopped, Recording Started/Stopped, **Event** (raw) | `meldStudio.*`, `meldStudioEvent._json` |
| PolyPop | none (sub-action only) | |

### 1.9 Elgato triggers (128)

| Product | Count | Highlights |
|---|---|---|
| **Camera Hub** | 68 | Connected/Disconnected. Webcam (40): Activated/Deactivated, Zoom/Pan/Tilt/Focus/ISO/Shutter/White-Balance/Brightness/Contrast/Saturation/Sharpness changes, auto-exposure/auto-focus/auto-WB toggles, Flip/Mirror, Snapshot Taken, Lens/AR Lens/Bitrate (EpocCam). Prompter (26): Mode, Brightness, Font, Font Size, Scroll Speed, Margins, Colors, Crosshair, Selected Script/Chapter/Channel, Auto-scroll. Variables follow a `value`/`oldValue` pattern, for example `zoomPercent`/`oldZoomPercent`. |
| **Wave Link** (1.x/2.x) | 20 | Output switched, volume, mute and selected output; input volume, mute and name; mic gain, balance, mute, output volume and settings; filter added, changed, deleted and bypass; level meters |
| **Wave Link 3.x** (1.1.0) | 19 | Channels (5), Inputs (4: device updated, devices changed, gain, DSP effects), Mixes (2), Outputs (3), System (Focused App Changed, Level Meter Updated, Create Profile Requested), Connected/Disconnected |
| **Elgato Studio** (1.1.0) | 9 | Connected/Disconnected, Recording Changed/Finished, Snapshot Taken, Preview volume, mute and fullscreen, Level Changed |
| **Control Center** (1.1.0, Key Lights and similar) | 9 | Device Added/Removed/Renamed/Configuration Changed, Power State Changed/Powered On/Off, Brightness Changed, Temperature Changed |
| **Stream Deck** | 3 | Connected, Disconnected, Stream Deck Info. Button presses arrive as action executions from the Streamer.bot Stream Deck plugin, not as triggers. |

### 1.10 Third-party integration triggers (97)

Grouped per integration in §3. Summary:

- CrowdControl 9
- DonorDrive 3
- Fourthwall 13
- HypeRate.io 4
- Ko-fi 5
- OBSBOT 5
- Pally.gg 3
- Patreon 5
- Pulsoid 1
- Shopify 2
- Speaker.bot 2
- StreamElements 4
- StreamLoots 4
- Streamer.bot Custom Webhook 1
- Streamer.bot Remote 3
- Streamlabs 5
- Throne 3
- T.I.T.S. (Throwing System) 8
- TipeeeStream 1
- TreatStream 1
- VTube Studio 11
- VoiceMod 4

---

## 2. Streaming platforms

Currently supported: **Twitch, YouTube and Kick** (https://streamer.bot/features#platforms, https://docs.streamer.bot/guide/platforms/twitch).

Platform history:

| Platform | Status | When | Why / source |
|---|---|---|---|
| Twitch | Supported since day 1 | 0.0.x | |
| YouTube | Supported | Added v0.1.8 | https://streamer.bot/features |
| Kick | Supported, free | Added v1.0.0 (2025-08-20); originally planned as a paid perk | https://streamer.bot/blog/announcements/to-the-community |
| **Trovo** | **Removed** | Added v0.2.3; removed v1.0.5 (Aug 2026) | "Trovo is no longer offering Live Streaming services." C# stubs remain until release. (https://streamer.bot/changelogs/v1.0.7) |
| **VStream** | **Removed** | Added v0.2.3; removed v0.2.4 | https://streamer.bot/changelogs/v0.2.4 |
| TikTok | Not supported | | No usable public API. TikFinity provides a 3rd-party bridge. (https://docs.streamer.bot/faq/platform-support) |

### 2.1 Capability matrix

| Capability | Twitch | YouTube | Kick |
|---|---|---|---|
| Event transport | **EventSub WebSocket**, direct from the PC to `wss://eventsub.wss.twitch.tv` **(observed in v1.0.7 binary)**. PubSub was removed in 1.0.0 and IRC in 1.0.5. | YouTube Data API v3 via the Streamer.bot Google project. **"PushAPI"** chat since 1.0.0 is gRPC `liveChatMessages.streamList` (`Streamer.bot.YouTube.Grpc.dll` **observed**; mapping to streamList **(inferred)**). Plus polling for subscribers (10 s), statistics and broadcasts. | **Webhooks** from Kick, received by Streamer.bot's cloud and relayed to your PC over Socket.IO (§4) |
| Read chat | Yes | Yes | Yes (can be delayed, because Kick webhooks are not always timely; https://docs.streamer.bot/faq/platform-support) |
| Send chat | Yes; broadcaster or bot, with fallback; Reply; Announcement; Send-and-Pin (1.0.5) | Yes; broadcaster or bot, with fallback; to a specific or latest monitored broadcast | Yes; send and reply, broadcaster or bot (C# `useBot`/`fallback`) |
| Whispers | Read and send (bot or broadcaster) | No | No |
| Moderation actions | Ban, timeout, untimeout, unban, warn, mod/VIP add/remove, suspicious-user status, chat modes, Shield Mode, clear chat, delete message, AutoMod events, unban requests, blocked terms | Ban, timeout | Ban, timeout, unban, untimeout; delete message (C# `KickDeleteMessage`) |
| Channel management | Title, game, tags, branded content, markers, clips (+download URLs), commercials and ad snooze, raids, shoutouts, polls, predictions, Guest Star, rewards CRUD (owned by the bot), sub counter, pinned messages | Title, description, privacy (Stream Info), polls create/end | Title, category, native Channel Rewards CRUD (maximum 15, no realtime updates) |
| Monetization events | Bits/cheers, subs/resubs/gifts/bombs/upgrades, Hype Train, Power-Ups, Custom Power-Ups, Charity, Goals, Watch Streaks | Super Chat, Super Sticker, Jewels, Memberships, Gifted memberships, Milestones | Subs, gifts, mass gifts, Kicks Gifted, Channel Rewards |
| Present-viewers list | Live chatter list | Activity-based (API lacks a viewer list) | Activity-based |
| 3rd-party emotes | BTTV, 7TV, FFZ (`api.frankerfacez.com`, `events.7tv.io`, `sockets.betterttv.net` **observed**) | YouTube, BTTV, 7TV, Twemoji | 7TV |
| Command source | Message, Whisper, Sub message, Resub message | Message | Message |
| Accounts | Broadcaster (required) and Bot (optional; "limited scope, can only send chat messages or whispers") | Broadcaster (required; brand accounts must be selected after the owner login) and Bot (optional) | Broadcaster (required) and Bot (optional; `KickGetBot()` C# **(inferred from C# API)**) |
| Multichat | All three appear in the built-in chat window and the `chat.streamer.bot` dock/overlay | | |

---

## 3. Third-party integrations

**Connection type legend**

- **LOCAL-WS**: WebSocket or local API to an app on this PC or LAN.
- **VENDOR-CLOUD**: a direct outbound connection from the PC to the vendor's cloud (WebSocket, Socket.IO, SSE or REST polling).
- **SB-RELAY**: the vendor sends webhooks to streamer.bot servers, which forward them to the app over the Streamer.bot Website socket. This requires a Streamer.bot account login.
- **OUT-ONLY**: the app only sends outbound requests.

### 3.1 Broadcasting software ("Stream Apps")

| Integration | Connection | Auth | Triggers | Sub-actions (manifest) | Source |
|---|---|---|---|---|---|
| **OBS Studio** | LOCAL-WS: obs-websocket **v5** (default port 4455) or **v4.9** compat (4444). Multiple instances, LAN supported, Default/Force connection, auto-reconnect. | obs-websocket password | 9 | 29: scenes, sources, filters, groups, media, browser URL, GDI text, image/media file, color source, flip/rotate, audio tracks, mute, replay buffer, record chapter, screenshot, stream/record, **Raw** request (`obs-raw.streamer.bot` helper) | https://docs.streamer.bot/guide/stream-apps/obs-studio |
| **Streamlabs Desktop** | LOCAL-WS: Remote Control API (port 59650). Multiple instances. Supports horizontal and vertical sources since 0.2.5. | Remote Control token | 7 | 16: scenes, sources, filters, groups, browser URL, GDI text, flip/rotate, mute, stream/record | https://docs.streamer.bot/guide/stream-apps/streamlabs-desktop |
| **Meld Studio** | LOCAL-WS: Meld's API; enable "Allow remote connections". Multiple instances. Added v0.2.5. | none | 8 | 15: show scene, layer visibility, browser layer URL, media layer source, text, effects, track mutes, record clip, screenshots (horizontal and vertical), stream/record, status | https://docs.streamer.bot/guide/stream-apps/meld-studio |
| **PolyPop** | LOCAL-WS: community WebSocket plugin (127.0.0.1:9652) | none | 0 | 1: Trigger Alert | https://docs.streamer.bot/guide/stream-apps/polypop |

### 3.2 Elgato family

| Integration | Connection | Triggers | Sub-actions | Notes |
|---|---|---|---|---|
| **Stream Deck** (plugin) | LOCAL-WS: Streamer.bot hosts a "Stream Deck Server" (default 127.0.0.1:8059). The official plugin `bot.streamer.streamdeck` connects to it. Bidirectional: keys run actions, and actions update keys. Supports dials and Stream Deck+. | 3 | 8: Set Background/State/Title/Value, Show Alert/OK, Toggle State, **Execute MCP Action** (Stream Deck MCP, 1.0.5) | https://streamdeck.streamer.bot, https://docs.streamer.bot/guide/integrations/elgato-stream-deck |
| **Wave Link 1.x/2.x** | LOCAL-WS to the Wave Link app (minimum version 1.11.1) | 20 | 16: mic/input/output volume, mute, gain, balance, filters, monitor device | https://docs.streamer.bot/guide/integrations/elgato-wave-link |
| **Wave Link 3.x** (1.1.0) | LOCAL (inferred) | 19 | 20: channels, inputs, mixes, outputs, add app to channel, clipguard, low-cut | https://streamer.bot/changelogs/v1.1.0 |
| **Camera Hub** | LOCAL to the Camera Hub app (Facecam, EpocCam, Prompter) | 68 | 34: webcam, EpocCam and Prompter controls | https://docs.streamer.bot/guide/integrations/elgato-camera-hub |
| **Elgato Studio** (1.1.0) | LOCAL (inferred) | 9 | 9: recording, preview, snapshot | 1.1.0 changelog |
| **Control Center** (1.1.0) | LOCAL (inferred) | 9 | 3: brightness, power, temperature (Key Light and similar) | 1.1.0 changelog |

### 3.3 Donation, tip, commerce and membership services

| Integration | Connection | Auth / setup | Triggers (key variables) | Sub-actions |
|---|---|---|---|---|
| **StreamElements** | VENDOR-CLOUD: Socket.IO `wss://realtime.streamelements.com` plus the REST `api.streamelements.com/kappa/v2` **(observed)** | OAuth ("Connect to StreamElements") | Tip (`tipUsername`, `tipAmount`, `tipCurrency`, `tipMessage`); Merch (`merchItems`, `merchItem#.*`); Connected/Disconnected | none |
| **Streamlabs** (web) | VENDOR-CLOUD: Socket.IO `sockets.streamlabs.com` plus `streamlabs.com/api/v2.0` **(observed)** | Changed from the Socket API token to **OAuth** in v0.2.5; the guide page still shows the token flow (https://streamer.bot/changelogs/v0.2.5) | Donation (`donationFrom`, `donationAmount`, `donationCurrency`, `donationFormattedAmount`, `donationMessage`); Merchandise; Charity Donation; Connected/Disconnected | Spin Wheel |
| **Ko-fi** | **SB-RELAY** (webhook) | Create the webhook on streamer.bot, paste its URL into Ko-fi, then paste the Ko-fi verification token back on streamer.bot | Donation, Subscription, Resubscription, Shop Order, Commission (`from`, `amount`, `currency`, `message`, `isPublic`, `tier`, `item#`) | none |
| **Patreon** | **SB-RELAY** (webhook) | Register the webhook in the Patreon portal and store its secret on streamer.bot | Follow Created/Deleted, Pledge Created/Updated/Deleted (Patreon JSON:API user, member and tier attributes) | none |
| **Fourthwall** | **SB-RELAY** (webhook) | As above, with a shop secret | 13: Donation, Order Placed/Updated, Gift Purchase, Gift Draw Started/Ended, Product Created/Updated, Subscription Purchased/Changed/Expired, Newsletter Subscribed, Thank You Sent (`fw.*`) | none |
| **Shopify** | **SB-RELAY** (webhook) | Store webhook (Order creation or Order payment) | Order Created, Order Paid (`shopify.*`) | none |
| **Throne** (1.0.5) | **SB-RELAY** (webhook) | URL only | Gift Purchased, Contribution Purchased, Gift Crowdfunded (`throne.*`) | none |
| **TipeeeStream** | VENDOR-CLOUD: `api.tipeeestream.com` **(observed)** | OAuth with a localhost callback (`http://localhost:8468/` in the TipeeeStream module, **observed**). Guide is WIP. | Donation (`username`, `amount`, `fees`, `currency`, `message`) | none |
| **TreatStream** | VENDOR-CLOUD: Socket.IO `nodeapi.treatstream.com` **(observed)** | Account login (an "Authenticated" event exists); guide is WIP | Treat (`username`, `type`, `message`) | none |
| **DonorDrive** (Extra Life, StackUp and other charities) | VENDOR-CLOUD: REST `api.donordrive.com` or a custom endpoint. Polling **(inferred)**. Participant or team. | Choose a charity provider and participant/team ID; no auth | Donation, Incentive, Profile Updated (`donorName`, `donorAmount`, `donorMessage`, `raised`, `goal`, `incentive.*`) | none |
| **Pally.gg** | VENDOR-CLOUD: `wss://events.pally.gg?...channel=firehose` **(observed)** | API key | Campaign Tip (`tip*` gross, net and fee), Connected/Disconnected | none |
| **StreamLoots** (1.0.5) | VENDOR-CLOUD: SSE `widgets.streamloots.com/alerts/{id}/media-stream` **(observed)** | Paste the Alerts Widget URL | Pack Purchased, Pack Gifted, Card Redeemed, Reaction | none |
| **Tiltify** | **Undocumented.** `v5api.tiltify.com/oauth/` and a "Tiltify Service" with a localhost callback are present in the v1.0.7 binary **(observed)**. No docs, triggers or sub-actions are listed. | OAuth (observed) | none in the manifest | none |

### 3.4 Interactive, game and VTuber integrations

| Integration | Connection | Auth | Triggers | Sub-actions |
|---|---|---|---|---|
| **Crowd Control** | VENDOR-CLOUD: `wss://pubsub.crowdcontrol.live` plus `openapi.crowdcontrol.live` **(observed)** | Paste the personal **Overlay URL** from the CC app (acts as a token) | 9: Game Session Start/End, Effect Request/Success/Failure, Timed Effect Started/Updated/Ended, Coin Exchange (`effect.*`, `game.*`, `gamePack.*`, `price`, `requestID`) | none |
| **VTube Studio** | LOCAL-WS (localhost:8001); approve the plugin in VTS | Plugin token | 11: Model Loaded/Unloaded, Model Clicked, Hotkey Triggered, Model Animation, Item Event, Background Changed, Tracking Status Changed, Model Config Changed, Connected/Disconnected | 10: hotkeys, expressions, load/move model, color tint, **Send Raw Request** |
| **T.I.T.S.** (Twitch Integrated Throwing System) | LOCAL-WS (separate API and events sockets) | none | 8: Item Hit (`itemName`, `strength`, `direction.x/y/z`), Trigger Activated/Ended, 5 connection triggers | 6: Throw Item(s) (by name), Activate Trigger (by name) |
| **mtion studio** | LOCAL (process detection; "no service to connect to") | none | 0 | 2: Get Active Clubhouse, Send Trigger |
| **StreamFog** (1.0.5) | VENDOR-CLOUD: `api.streamfog.com/api/v3` **(observed)**, using a "Remote Key" from the StreamFog app | Remote Key | 0 | 8: Lenses, Outfits, Video Backgrounds (by ID or name), Reset Look |
| **VoiceMod** | LOCAL-WS: Voicemod Control API; probes multiple ports. Supports v2.48–2.51 and v3.12+. | Local | 4: Connected, Disconnected, Voice Loaded, Soundboard Changed | 12: select voice (ID, name, random), voice changer, hear-my-voice, mute, background effect, censor (v2 only), play sound, stop sounds |
| **Speaker.bot** (sister TTS app) | LOCAL-WS (default 127.0.0.1:7580, LAN supported) plus a UDP speak option | none | 2: Connected/Disconnected | 7: Speak, Speak (UDP), Stop, Clear Pending, Set Enabled/Paused, Activate VoiceGate Profile |
| **Lumia Stream** (smart lights) | LOCAL: Lumia Developers API **(inferred)** | API token from Lumia | 0 | 3: Send Command, Set Color, Set To Default |
| **OBSBOT** (1.1.0-alpha, undocumented) | LOCAL to OBSBOT Center **(inferred)** | ? | 5: Connected/Disconnected, Device Connected/Disconnected, Device Sleep State Changed | 11: Gimbal move/reset/position, Zoom level/min/max, Sleep State, Take Snapshot |

### 3.5 Health and biometrics

| Integration | Connection | Auth | Triggers |
|---|---|---|---|
| **HypeRate.io** | VENDOR-CLOUD: `wss://app.hyperate.io/ws/` **(observed)** | HypeRate ID (`internal-testing` gives fake data) | Heart Rate Pulse (`heartRate`), Twitch Clip Created (`clipUrl`, when HypeRate auto-clips), Connected/Disconnected |
| **Pulsoid** | VENDOR-CLOUD: `wss://dev.pulsoid.net/api/v1/data/real_time` **(observed)** | OAuth with a localhost callback (`http://localhost:8578/` **observed**) | Heart Rate Pulse (`heartRate`, `measuredAt`) |

### 3.6 Messaging, automation and utility

| Integration | Connection | Triggers | Sub-actions | Notes |
|---|---|---|---|---|
| **Discord** | OUT-ONLY: Discord **webhook URL** (HTTP POST) | 0 | 1: Basic Webhook (text, username, avatar, image URL) | **No bot or gateway connection, so Discord events cannot be received.** (https://docs.streamer.bot/guide/integrations/discord) |
| **IFTTT** | **SB-RELAY**, bidirectional, via the "Streamer.bot" service on IFTTT | Incoming IFTTT "Execute Action" or "Execute Action (JSON)" runs a chosen action with `iftttText` or top-level JSON keys as args. Not a trigger; it executes the action directly. | 2: IFTTT Notification (Basic), IFTTT Notification (Advanced). These act as IFTTT "If This" triggers, polled by IFTTT every 5 min (Pro) or hourly (Free). | https://docs.streamer.bot/guide/integrations/ifttt, https://ifttt.com/streamerbot |
| **Pronouns** (alejo.io) | VENDOR-CLOUD: REST `api.pronouns.alejo.io/v1/` **(observed)** | 0 | 1: Add Pronouns for User | Built in since 0.2.3 |
| **Streamer.bot Remote** | **SB-RELAY**: instance-to-instance on the same account | 3: Remote Instance Connected/Disconnected/Trigger (`instanceId`, `instanceName`, `instanceVersion`, `instanceOs`, plus forwarded args) | 3: Send Remote Trigger, Send Remote Signal, Wait For Remote Signal | Event history and replay on the website (https://streamer.bot/changelogs/v0.2.4) |
| **Streamer.bot Decks** | **SB-RELAY**: web-based decks hosted by streamer.bot; can be shared privately with moderators | 0 | 10: Deck Notify, Item Set Background/Icon/State/Title/Value/Toggle, Page Next/Prev/Set | https://streamer.bot/features#extended |
| **Streamer.bot Custom Webhook** (supporter perk) | **SB-RELAY**: generic webhook from any site, forwarded to the app | 1: Custom Webhook | none | https://streamer.bot/changelogs/v1.0.0 |

### 3.7 Integrations the docs do NOT have

| Asked about | Status |
|---|---|
| **Twitter/X** | **No native integration in any documented version** (see §5). Possible only via IFTTT or custom HTTP/C#. |
| Spotify | No integration (only community extensions, (inferred)) |
| Philips Hue / Nanoleaf / Govee | No direct integration. Smart lights go through **Lumia Stream**. |
| LIOranBoard / SAMMI | No integration |
| Discord bot (receive events) | No; outbound webhook only |
| TikTok | No; use TikFinity (3rd party) |
| Touch Portal, VoiceAttack, Deckboard, LoupeDeck, Macro Deck | Community plugins that call Streamer.bot's own APIs (https://streamer.bot/features#plugins) |

---

## 4. Cloud and online routing: what the project owner most likely means

### 4.1 Summary

Streamer.bot markets itself as a **local** bot: "all connections are made directly from your local PC to any configured external services … your bot does not depend on a central 3rd party service" (https://docs.streamer.bot/get-started/introduction).

There is, however, an **optional but increasingly important cloud component: the "Streamer.bot Website Integration."** Here is how it works:

1. The desktop app logs into a **Streamer.bot account**. This is OpenID Connect against `https://auth.streamer.bot/auth/realms/streamer.bot`, a Keycloak realm. Social login is available via Twitch, Kick, Google or Discord **(observed in v1.0.7 binary and on the https://streamer.bot/auth/login page)**.
2. The app then keeps a persistent **Socket.IO WebSocket to `wss://streamer.bot/socket.io/`** **(observed)**. It also uses the REST API `https://streamer.bot/api/` **(observed)**.
3. streamer.bot's backend acts as a **public, internet-reachable endpoint and relay**. Third parties push data to it, and it forwards that data over the socket to the user's instance. The app is behind NAT and has no public URL, so this is the only way to receive webhooks.

The changelog states it explicitly: "Due to the nature of webhooks, the website acts as a relay for these services, meaning you need to have your Streamer.bot logged in to be able to receive them." (https://streamer.bot/changelogs/v1.0.5)

### 4.2 Everything that routes through streamer.bot servers

| Feature | Direction | What is relayed | Why it needs the cloud | Free or perk | Source |
|---|---|---|---|---|---|
| **Kick platform events** | Kick → `kick.streamer.bot` → app | All Kick events (chat, follows, subs, gifts, rewards, Kicks, moderation, stream state) | Kick's public API delivers events only by **webhooks** to the app-level URL registered for Streamer.bot's Kick developer app. The user must log into Kick in the app **and** link the same Kick account on streamer.bot; the website shows subscription status and "Kick API Delay". Kick has a webhook-subscription cap that Streamer.bot had to ask Kick to raise (v1.0.1). The app uses `https://kick.streamer.bot/api/` and a Socket.IO client **(observed)**. Whether Kick OAuth token exchange or refresh also passes through `kick.streamer.bot` is **(inferred, unconfirmed)**. | Free | https://docs.streamer.bot/guide/platforms/kick, https://streamer.bot/changelogs/v1.0.1 |
| **Ko-fi, Patreon, Fourthwall, Shopify, Throne** | Vendor → streamer.bot webhook URL → app | Webhook payloads. The website verifies the vendor secret ("Verify Secret" field), and the app deserializes the vendor payload: the Ko-fi and Patreon models are in `Streamer.bot.Website.dll` **(observed)**. | Webhooks need a public HTTPS endpoint | Free | Integration guides §3.3 |
| **Custom Webhooks** | Any site → streamer.bot → app ("Custom Webhook" trigger) | Arbitrary webhook bodies; "monitored for abuse" | Same as above | **Supporter perk** (Patreon "Thank You" tier, $3/mo) | https://streamer.bot/changelogs/v1.0.0 |
| **IFTTT** | Both directions | Execute Action requests in; Notification sub-action payloads out | IFTTT talks to a cloud service API, not to your PC | Free | https://docs.streamer.bot/guide/integrations/ifttt |
| **Remote Triggers and Signals** | Instance ↔ instance | Event name plus arguments | Instances on different PCs or networks | Free | https://streamer.bot/changelogs/v0.2.4 |
| **Streamer.bot Decks** | Web deck ↔ app | Button presses in; deck UI updates out | Hosted web UI usable from any device or by moderators | Free (perks raise limits) | https://streamer.bot/changelogs/v1.0.0 |
| **Cloud Backups** | App → streamer.bot storage | An encrypted backup of the configuration. Can optionally include `auth.db`, which contains platform OAuth tokens. | Off-machine restore | **Supporter perk** | https://docs.streamer.bot/guide/core/backup |
| **Release channels and perks** | App ↔ streamer.bot | Update checks (alpha and beta require login), Patreon tier entitlement, Discord beta role | Licensing and gating | | https://docs.streamer.bot/get-started/installation |

These do **not** route through streamer.bot servers:

- Twitch (direct EventSub/Helix)
- YouTube (direct Google APIs)
- OBS, Streamlabs Desktop, Meld, Elgato, VTube Studio, VoiceMod, T.I.T.S., Speaker.bot, Lumia (local)
- StreamElements, Streamlabs, Crowd Control, HypeRate, Pulsoid, Pally.gg, StreamLoots, TreatStream, TipeeeStream, DonorDrive, StreamFog (direct to vendor clouds)

Other online dependencies:

- **Hosted web UIs that connect back to the local WebSocket server.** `chat.streamer.bot` (multichat dock, overlay and event feed) is served from the cloud but talks **browser → your local WebSocket server**. The docs warn browsers may block this and suggest Tailscale, Cloudflare Tunnel or ngrok. `obs-raw.streamer.bot` and `editor.streamer.bot` appear to be similar hosted tools. Their data path is local, not relayed. (https://docs.streamer.bot/guide/extra-features/chat)
- **YouTube's shared quota.** Every Streamer.bot user goes through **one Google Cloud project and OAuth client**, so the daily **API quota is shared across all users** ("applies to all users at the same time … reset daily at 00:00 PST"). This is a central bottleneck even though the data is not relayed. (https://docs.streamer.bot/faq/youtube-quota) The privacy policy says YouTube tokens stay in a local settings file. (https://streamer.bot/privacy)
- **Crash telemetry.** `Sentry.dll` and `Sentry.Serilog.dll` ship with the app **(observed)**, which suggests crash reporting to Sentry (inferred).
- **Startup switch.** `/nologin` skips logging into the website integration "if the Streamer.bot Website integration is down." (https://docs.streamer.bot/guide/advanced/switches)

### 4.3 Most likely meaning of "routing through an online path"

**(inferred)** Most likely: **the Streamer.bot Website Integration socket relay.** It is visible in the UI as the bottom-left "Login," and it is mandatory for **Kick** and for every **webhook-based integration** (Ko-fi, Patreon, Fourthwall, Shopify, Throne, Custom Webhooks), as well as for IFTTT (including "post to X"), Remote instances and Decks.

Second possibility: `chat.streamer.bot`, which looks like an online service but is just a hosted page talking to localhost.

**Implication for our app:** to offer webhook integrations and Kick without asking users to run tunnels, we need our own internet-facing relay with per-user routing, secret verification and a persistent client socket. Alternatively, we could offer a "bring your own tunnel" mode.

---

## 5. Twitter/X

**Finding: Streamer.bot does not support, and per all available records never natively supported, Twitter/X.**

- No integration page, trigger or sub-action for Twitter/X exists in the current docs or the 1.1.0-alpha.17 manifests. The only "twitter" string in the docs is a Patreon user's `social_connections.twitter` field.
- None of the changelogs from v0.1.1 through v1.1.0, nor the archived changelogs back to v0.0.30, mention a Twitter integration being added or removed.
- The v1.0.7 binary contains no Twitter or X API client; the only `x.com` and `twitter.com` strings are social links **(observed)**.
- The dev's own changelogs joke "Dead Blue Bird: twitter.com/streamerdotbot" (https://streamer.bot/changelogs/v1.0.4), but that refers only to their social account.
- **The supported way to post to X is IFTTT.** You can use an IFTTT applet such as "Post on X when Streamer.bot sends a notification," fired by the IFTTT Notification sub-action. That route goes through the Streamer.bot Website relay and then IFTTT (https://ifttt.com/connect/streamerbot/twitter). There are also IFTTT applets in the other direction, for example "run Streamer.bot action when you post a new tweet." Users can also call the X API themselves with Fetch URL, HTTP or C#.
- **Possible source of the owner's belief (inferred):**
  - Elgato's Stream Deck **removed its own Twitter/X actions in Stream Deck 6.4** after X API changes (https://help.elgato.com/hc/en-us/articles/18565628479373).
  - Streamlabs Desktop has a built-in "Tweet when live."
  - Either could be conflated with Streamer.bot.

I could not find any removal date because there was nothing to remove.

---

## 6. Authentication model per platform and integration

| Service | Accounts | Mechanism | Streamer.bot-hosted service involved? | Notes / scopes |
|---|---|---|---|---|
| **Streamer.bot account** | One per user | OIDC (Keycloak at `auth.streamer.bot`), browser login with a local callback (`http://localhost:7121/` **observed**). Social login via Twitch, Kick, Google or Discord. | **Yes**: this *is* the hosted service | Required for Kick, webhooks, IFTTT, Remote, Decks, Cloud Backup, alpha/beta updates. Link Patreon on the website for perks. |
| **Twitch** | Broadcaster (required) and Bot (optional, chat and whispers only) | OAuth in the browser with a **local redirect callback**; since 1.0.5 a new auth handler uses Kestrel for redirect callbacks, adds a "Copy Login URL" option for any browser, a 3-minute timeout and cancel (https://streamer.bot/changelogs/v1.0.7). Tokens live in local `auth.db`. Exact localhost port **(inferred: one of the observed fixed `localhost:` callback ports)**. | No (direct to Twitch) | Scopes have grown over time. Named in changelogs: `channel:manage:ads`, `channel:read:ads`, `user:read:chat`, `user:write:chat`, `user:manage:blocked_users` (0.2.3); `user:read:emotes`, `moderator:manage:unban_requests`, `moderator:read:moderators`, `moderator:read:vips`, `moderator:read:suspicious_users`, `moderator:manage:warnings` (0.2.4); more in 1.0.3; suspicious-user management in 1.0.5. A scope mismatch prompts re-login. |
| **YouTube** | Broadcaster (required) and Bot (optional) | "Sign in with Google" OAuth, **installed-app flow with a local callback** (`http://127.0.0.1:8919/` **observed**). Tokens are stored locally (https://streamer.bot/privacy). Brand accounts: log in with the owning account, then pick the brand. | No relay, but a **shared Google project and quota** | The changelog says "only 1 main scope" is needed since 0.2.4. The binary references `youtube`, `youtube.force-ssl`, `youtube.readonly`, `youtube.channel-memberships.creator`, `userinfo.*` and others **(observed; the actual requested set is unconfirmed)**. |
| **Kick** | Broadcaster (required) and Bot (supported in the C# API) | OAuth via `id.kick.com` with a local callback **(observed)**, plus **linking the same Kick account on streamer.bot** | **Yes** (`kick.streamer.bot` relay; §4) | New scopes in 1.0.3 and 1.0.5 force re-login |
| OBS / Streamlabs Desktop / Meld | n/a | Host, port, and password or token | No | LAN multi-PC supported |
| StreamElements, Streamlabs, Pulsoid, TipeeeStream, TreatStream, (Tiltify) | One account | OAuth with **local callbacks** (Pulsoid `localhost:8578`, TipeeeStream `localhost:8468` **observed**). OAuth **client credentials for several providers are embedded in the shipped app** **(observed; values not reproduced)**, so token exchange happens locally. | No | |
| Ko-fi, Patreon, Fourthwall, Shopify, Throne, Custom Webhook | n/a | Vendor webhook secret or verification token, stored **on streamer.bot** | **Yes** | |
| Pally.gg | n/a | API key | No | |
| Crowd Control / StreamLoots / HypeRate / StreamFog | n/a | Bearer-like URL or ID (Overlay URL, Alerts Widget URL, HypeRate ID, StreamFog Remote Key) | No | |
| VTube Studio, VoiceMod, T.I.T.S., Wave Link, Camera Hub, Speaker.bot, Lumia (token) | n/a | Local app approval or local token | No | |
| Discord | n/a | Webhook URL | No | |
| IFTTT | Streamer.bot account linked on IFTTT | IFTTT ↔ Streamer.bot OAuth | **Yes** | |

---

## 7. Parity checklist (implications for our app)

1. **Platform event transport**
   - Twitch: EventSub WebSocket, directly from the client (no server needed).
   - YouTube: Data API plus gRPC `streamList` chat. Budget for quota: a shared project quota is a real operational risk. Consider letting users bring their own Google project and keys.
   - Kick: needs a **public webhook receiver plus a relay** (or Kick WebSockets, if Kick ever ships them; https://github.com/KickEngineering/KickDevDocs/issues/20).
2. **A relay service** is needed for Kick and for webhook-only vendors (Ko-fi, Patreon, Fourthwall, Shopify, Throne, generic webhooks), IFTTT-style automation, remote instances and web decks.
3. **Trigger model to replicate:**
   - numeric event IDs
   - hierarchical categories
   - Select/Range/Toggle/Text filters with an "Any" option and Always-Run
   - test and simulate
   - custom triggers from scripts
   - WebSocket event schema per event
4. **Variable conventions:**
   - common per-platform user and broadcaster groups
   - `rawInput*` triplets
   - `#`-indexed lists
   - dotted namespaces
   - `isTest`, `__source`, `eventSource`
   - Consider a cross-platform normalized user model, since Streamer.bot duplicates `user`, `userName` and `userId` per platform.
5. **Local integrations** are mostly WebSocket clients: OBS v5 (v4 is legacy), Streamlabs Desktop, Meld, VTube Studio, VoiceMod, T.I.T.S., Elgato apps and Speaker.bot. The Stream Deck plugin connects *to us*.
6. **Missing in Streamer.bot (differentiation opportunities):**
   - native X/Bluesky posting
   - a Discord bot (receiving events)
   - TikTok
   - Spotify
   - direct smart-light vendors

---

## 8. Open questions for the project owner

1. "Online routing": did you mean Kick, the webhook integrations (Ko-fi, Patreon and others), IFTTT, Decks/Remote, or the hosted `chat.streamer.bot` dock? Which of these must we replicate at launch? Each requires us to run a public relay service.
2. Twitter/X: Streamer.bot never had a native integration (only IFTTT). Do you want native X posting? Bluesky? Note X API costs.
3. Are we willing to operate cloud infrastructure (relay, accounts, backups), or must the app be 100 % local with "bring your own tunnel or webhook URL"?
4. YouTube: should we ship a shared Google project (quota risk, verification burden) or require users to supply their own API project?
5. Kick: do we accept webhook latency via a relay, or wait for Kick WebSockets?
6. Which 1.1.0-alpha features count as "parity": Points system, Cron, controller input, Whisper voice control, Wave Link 3, Elgato Studio and Control Center, OBSBOT?
7. Should Trovo-like dead platforms be skipped, and should we add TikTok (unofficial APIs carry ToS risk)?
8. Paid tiers: Streamer.bot gates Cloud Backups and Custom Webhooks behind Patreon. Will we have perks?
