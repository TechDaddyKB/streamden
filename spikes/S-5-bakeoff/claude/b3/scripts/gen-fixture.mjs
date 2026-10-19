// Generates src/fixture/subactions.json: a deterministic list of ~200 sub-action
// definitions grouped as category > group > item. Names are generic automation
// verbs written for this spike; nothing is copied from another product.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const catalogue = {
  Core: {
    Logic: ["If / Else", "Switch", "Break", "Wait for Condition", "Run Action", "Run Action by Name", "Enable Action", "Disable Action", "Set Action Queue State", "Comment"],
    Variables: ["Set Argument", "Set Global Variable", "Get Global Variable", "Unset Global Variable", "Increment Variable", "Set User Variable", "Get User Variable", "Clear User Variables"],
    Delays: ["Delay", "Random Delay", "Wait Until Time"],
    Files: ["Read Line From File", "Write To File", "Append To File", "Delete File", "Read Random Line"],
    Network: ["Fetch URL", "HTTP POST", "WebSocket Send", "UDP Send", "Open URL in Browser"],
    System: ["Run Program", "Send Keystroke", "Play Sound", "Show Desktop Notification", "Set Clipboard", "Log Message"],
    Scripting: ["Execute JavaScript", "Execute Python", "Evaluate Expression"],
    Math: ["Random Number", "Round Number", "Calculate"],
  },
  Twitch: {
    Chat: ["Send Message", "Send Reply", "Send Announcement", "Delete Message", "Clear Chat", "Set Emote-Only Mode", "Set Follower-Only Mode", "Set Slow Mode", "Set Subscriber-Only Mode", "Send Whisper", "Shoutout User"],
    Moderation: ["Timeout User", "Ban User", "Unban User", "Add Moderator", "Remove Moderator", "Add VIP", "Remove VIP", "Warn User", "Add Blocked Term"],
    Channel: ["Set Title", "Set Category", "Add Stream Marker", "Create Clip", "Run Commercial", "Start Raid", "Cancel Raid", "Set Tags"],
    "Channel Points": ["Create Reward", "Enable Reward", "Disable Reward", "Pause Reward", "Update Reward Cost", "Fulfil Redemption", "Refund Redemption"],
    Polls: ["Create Poll", "End Poll", "Archive Poll"],
    Predictions: ["Create Prediction", "Lock Prediction", "Resolve Prediction", "Cancel Prediction"],
    Users: ["Get User Info", "Get Follow Age", "Get Random Active User", "Check Subscriber Status"],
  },
  YouTube: {
    Chat: ["Send Message", "Delete Message", "Timeout User", "Ban User", "Set Slow Mode"],
    Broadcast: ["Set Title", "Set Description", "Insert Cue Point", "Get Live Broadcast"],
    Polls: ["Create Poll", "End Poll"],
  },
  Kick: {
    Chat: ["Send Message", "Delete Message", "Timeout User", "Ban User"],
    Channel: ["Set Title", "Set Category"],
  },
  OBS: {
    Scenes: ["Set Current Scene", "Set Preview Scene", "Trigger Studio Mode Transition", "Get Current Scene"],
    Sources: ["Set Source Visibility", "Toggle Source Visibility", "Set Source Position", "Set Source Scale", "Set Source Filter State", "Set Browser Source URL", "Refresh Browser Source", "Set Text Source", "Set Image Source File", "Set Media Source File"],
    Media: ["Play Media", "Pause Media", "Stop Media", "Restart Media", "Seek Media"],
    Audio: ["Set Input Mute", "Toggle Input Mute", "Set Input Volume", "Set Audio Monitor Type"],
    Output: ["Start Streaming", "Stop Streaming", "Start Recording", "Stop Recording", "Pause Recording", "Start Replay Buffer", "Save Replay Buffer", "Start Virtual Camera", "Stop Virtual Camera"],
    Advanced: ["Send Raw Request", "Take Source Screenshot", "Set Transition", "Set Transition Duration"],
  },
  "Streamlabs Desktop": {
    Scenes: ["Set Current Scene", "Set Source Visibility"],
    Output: ["Start Streaming", "Stop Streaming", "Start Recording", "Stop Recording", "Save Replay"],
  },
  Voice: {
    "Text to Speech": ["Speak Text", "Speak With Voice", "Stop Speaking", "Set Voice Speed", "Set Voice Volume", "Clear Speech Queue", "Pause Speech Queue"],
    "Speech to Text": ["Start Listening", "Stop Listening", "Wait for Phrase"],
  },
  VTubing: {
    "VTube Studio": ["Trigger Hotkey", "Set Expression", "Move Model", "Set Parameter Value", "Load Model", "Tint Art Mesh"],
    "Model Props": ["Spawn Item", "Remove Item", "Throw Item at Model"],
  },
  Overlays: {
    Alerts: ["Show Alert", "Queue Alert", "Skip Current Alert", "Pause Alert Queue"],
    Widgets: ["Show Widget", "Hide Widget", "Update Widget Data", "Send Overlay Event"],
  },
  Music: {
    Spotify: ["Play", "Pause", "Skip Track", "Previous Track", "Add to Queue", "Get Current Track", "Set Volume"],
  },
  "Hardware & Devices": {
    "MIDI": ["Send Note", "Send Control Change", "Send Program Change"],
    "Stream Deck": ["Set Button Title", "Set Button Image", "Set Button State"],
    "Smart Lights": ["Set Light Colour", "Set Light Brightness", "Run Light Effect"],
  },
  Integrations: {
    Discord: ["Post Webhook Message", "Post Embed"],
    "Donations": ["Get Recent Donations", "Get Donation Goal", "Set Donation Goal"],
    "Webhooks": ["Send Webhook", "Send Signed Webhook"],
  },
};

const slug = (s) => s.toLowerCase().replace(/&/g, "and").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

const keywordHints = {
  "Delay": ["wait", "sleep", "pause"],
  "Send Message": ["chat", "say", "post"],
  "Timeout User": ["mute", "purge"],
  "Set Current Scene": ["switch scene", "change scene"],
  "Speak Text": ["tts", "say"],
  "Execute JavaScript": ["js", "code", "script"],
  "Execute Python": ["py", "code", "script"],
  "Fetch URL": ["http get", "request", "api"],
};

const items = [];
for (const [category, groups] of Object.entries(catalogue)) {
  for (const [group, names] of Object.entries(groups)) {
    for (const name of names) {
      items.push({
        id: `${slug(category)}.${slug(group)}.${slug(name)}`,
        name,
        category,
        group,
        description: `${name} (${category} · ${group}).`,
        keywords: keywordHints[name] ?? [],
      });
    }
  }
}

const ids = new Set(items.map((i) => i.id));
if (ids.size !== items.length) throw new Error("duplicate sub-action id");

const out = fileURLToPath(new URL("../src/fixture/subactions.json", import.meta.url));
writeFileSync(out, JSON.stringify({ version: 1, items }, null, 2) + "\n");
console.log(`wrote ${items.length} sub-actions to ${out}`);
