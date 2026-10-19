#!/usr/bin/env node
/**
 * Deterministic catalog for the B-3 picker spike.
 * Names are a clean-room sample of the kind of sub-actions a streaming hub
 * exposes (Core, Twitch, YouTube, Kick, OBS, Streamlabs). They are not a
 * dump of any vendor's menu.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const catalog = {
  Core: {
    Logic: [
      "If / Else",
      "Switch",
      "While",
      "Break",
      "Loop Continue",
      "Delay",
      "Comment",
      "Random",
      "Group",
      "Set Weight",
      "Evaluate Expression",
      "Wait",
    ],
    Actions: [
      "Run Action",
      "Stop Action",
      "Enable Action",
      "Disable Action",
      "Get Action State",
      "Duplicate Action",
      "Run Action Inline",
      "Queue Action",
    ],
    Arguments: [
      "Set Argument",
      "Unset Argument",
      "Get Argument",
      "Increment Argument",
      "Set Argument If Missing",
      "Copy Argument",
    ],
    Globals: [
      "Set Global",
      "Get Global",
      "Unset Global",
      "Increment Global",
      "Persist Global",
      "Set User Global",
    ],
    Queues: [
      "Pause Queue",
      "Resume Queue",
      "Clear Queue",
      "Set Queue Blocking",
      "Get Queue Pending Count",
      "Pause All Queues",
    ],
    Files: [
      "Read File",
      "Write File",
      "Append File",
      "Tail File",
      "Watch File",
      "Delete File",
    ],
    Network: [
      "HTTP Request",
      "WebSocket Send",
      "UDP Send",
      "Open WebSocket",
      "Close WebSocket",
      "HTTP Download",
    ],
    System: [
      "Play Sound",
      "Text to Speech",
      "Show Toast",
      "Run Process",
      "Clipboard Set",
      "Open URL",
    ],
    Timers: ["Start Timer", "Stop Timer", "Reset Timer", "Set Timer Interval"],
    Users: [
      "Get User Info",
      "Set User Group",
      "Add User to Group",
      "Remove User from Group",
    ],
    Commands: [
      "Add Command",
      "Remove Command",
      "Enable Command",
      "Disable Command",
      "Set Command Cooldown",
      "Get Command State",
    ],
    Quotes: ["Add Quote", "Get Quote", "Delete Quote", "Edit Quote"],
    MIDI: ["Send MIDI Note", "Send MIDI CC", "All Notes Off"],
  },
  Twitch: {
    Chat: [
      "Send Message",
      "Send Announcement",
      "Send Reply",
      "Delete Message",
      "Clear Chat",
      "Send Whisper",
      "Set Emote Mode",
      "Set Slow Mode",
      "Set Follower Mode",
      "Timeout User",
    ],
    Channel: [
      "Update Title",
      "Update Category",
      "Set Stream Marker",
      "Create Clip",
      "Get Follower Count",
      "Set Stream Language",
      "Set Stream Tags",
      "Run Commercial",
    ],
    Moderation: [
      "Ban User",
      "Unban User",
      "Untimeout User",
      "Add Blocked Term",
      "Remove Blocked Term",
      "Warn User",
      "Shield Mode",
      "Clear Chat Messages",
    ],
    Rewards: [
      "Create Reward",
      "Update Reward",
      "Delete Reward",
      "Fulfill Redemption",
      "Cancel Redemption",
      "Pause Rewards",
    ],
    Polls: ["Create Poll", "End Poll", "Archive Poll", "Get Poll", "Terminate Poll"],
    Predictions: [
      "Create Prediction",
      "Lock Prediction",
      "Resolve Prediction",
      "Cancel Prediction",
      "Get Prediction",
    ],
    Raids: ["Start Raid", "Cancel Raid", "Send Shoutout", "Welcome Incoming Raid"],
    Subscriptions: [
      "Get Subscriber Count",
      "Check Subscription",
      "Set Sub Only",
      "Gift Sub Thanks",
      "Sub Counter Add",
    ],
    Ads: ["Start Ad", "Snooze Ad", "Get Ad Schedule", "Set Ad Snooze"],
    Charity: ["Get Charity Campaign", "Set Charity Goal", "Announce Charity"],
    "Guest Star": ["Invite Guest", "Remove Guest", "Set Guest Volume", "Move Guest Slot"],
  },
  YouTube: {
    Chat: [
      "Send Message",
      "Delete Message",
      "Timeout User",
      "Set Slow Mode",
      "Pin Message",
      "Reply to Super Thanks",
    ],
    Broadcast: [
      "Update Title",
      "Update Description",
      "Set Category",
      "Get Viewer Count",
      "Add to Playlist",
      "Set Thumbnail Note",
    ],
  },
  Kick: {
    Chat: [
      "Send Message",
      "Delete Message",
      "Timeout User",
      "Ban User",
      "Clear Chat",
      "Send Reply",
    ],
    Channel: ["Update Title", "Set Category", "Get Followers", "Send Announcement"],
  },
  OBS: {
    Scenes: [
      "Set Current Scene",
      "Create Scene",
      "Remove Scene",
      "Set Scene Name",
      "Get Current Scene",
      "Set Preview Scene",
    ],
    Sources: [
      "Set Source Visibility",
      "Set Source Mute",
      "Toggle Source Mute",
      "Set Source Volume",
      "Refresh Browser Source",
      "Set Text Source",
      "Screenshot Source",
      "Set Source Filter",
    ],
    Filters: ["Set Filter Enabled", "Set Filter Settings", "Add Filter", "Remove Filter"],
    Streaming: ["Start Streaming", "Stop Streaming", "Start Recording", "Stop Recording"],
    Replay: ["Save Replay", "Start Replay Buffer", "Stop Replay Buffer"],
    "Studio Mode": ["Enable Studio Mode", "Disable Studio Mode", "Trigger Transition"],
  },
  Streamlabs: {
    Scenes: ["Set Current Scene", "Create Scene", "Remove Scene", "Set Preview Scene"],
    Sources: [
      "Set Source Visibility",
      "Set Source Mute",
      "Set Text Source",
      "Set Source Volume",
      "Refresh Browser Source",
    ],
    Streaming: ["Start Streaming", "Stop Streaming", "Start Recording"],
  },
};

function slug(value) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

const items = [];
const seen = new Set();

for (const [top, groups] of Object.entries(catalog)) {
  for (const [sub, names] of Object.entries(groups)) {
    for (const name of names) {
      const id = `${slug(top)}.${slug(sub)}.${slug(name)}`;
      if (seen.has(id)) {
        throw new Error(`duplicate id ${id}`);
      }
      seen.add(id);
      items.push({
        id,
        name,
        category: [top, sub],
        description: `${name} in ${top} / ${sub}.`,
      });
    }
  }
}

if (items.length < 190 || items.length > 210) {
  throw new Error(`expected about 200 items, got ${items.length}`);
}

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "src", "fixture", "subactions.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(items, null, 2)}\n`);
console.log(`wrote ${items.length} sub-actions to ${out}`);
