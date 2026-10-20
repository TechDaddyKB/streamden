# Discord: application, bot user and server invite

Last verified against official docs: 2026-10-18; connection model re-verified 2026-10-19 (DEN-244, D-059). Part of the [owner runbooks](../README.md).

## Purpose

Discord has two integration levels in StreamDen:

1. **Webhook URL (default, P2).** The user pastes a channel webhook URL and the app posts "I'm live" messages. This needs **no developer application at all**. See step 0 below.
2. **Bring-your-own bot (optional).** Reads Discord channel chat into the multichat and can send or moderate. This needs a Discord **application with a bot user**, a bot token, one privileged intent, and an invite to a server. Discord OAuth cannot be done from a desktop app without a secret, so a shared bot is not planned.

Everything below is for the owner's own testing and for the wizard text we will give users.

## How users connect (D-059 applied)

- **Go-live posts (P2).** **Preferred target:** Discord's `webhook.incoming` consent: StreamDen opens Discord's authorization page in the browser, the user picks the channel, and the token response returns the webhook `id` and `token` ([OAuth2 webhooks](https://docs.discord.com/developers/topics/oauth2#webhooks), re-read 2026-10-19). Every Discord token call needs the client secret ("All calls to the OAuth2 endpoints require either HTTP Basic authentication or `client_id` and `client_secret`"), so the exchange runs in the hosted relay's token broker (D-019: later, timing Q-C12), never in the desktop. **Interim default and advanced fallback:** the user pastes a **channel webhook URL** (step 0). Both paths store the same webhook credential, so the connector accepts either.
- **Bot (P4, Q-S7):** the **invite link** opens in the browser and is a consent-only flow (no token exchange). The bot token itself is still pasted once from the user's own application (bring-your-own bot, D-024 research), because Discord has no secret-free OAuth and a shared bot would need hosting and verification.

## Prerequisites

- [ ] A Discord account (use the project identity) with 2FA enabled.
- [ ] A **test server** you own. Create one: Discord client, click the **+** at the bottom of the server list, **Create My Own**, **For me and my friends** (labels: verify on screen). Add channels such as `#go-live` and `#bot-test`.
- [ ] Final product name (Q-B1) for the application name. The name is the bot's default display name.

## Cost and limits

- **Free.**
- REST: 50 requests per second global. Gateway: 120 events per 60 s per connection, **1,000 IDENTIFY calls per 24 hours**.
- Privileged intents: apps with fewer than 10,000 users enable them in the Developer Portal. At scale (100+ servers) an app must be verified and the intent approved.
- Bots that are not marked public can only be added by their owner.

## Step-by-step

### 0. Webhook-only path (no application needed)

1. In your Discord server: **Server Settings > Integrations > Webhooks > New Webhook** (labels: verify on screen).
2. Choose the channel, give it a name, click **Copy Webhook URL**.
3. The URL is a credential: anyone with it can post to that channel. Store it in the StreamDen settings, never in the repo.

### 1. Create the application

4. Open <https://discord.com/developers/applications> and log in. Accept the Developer Terms if shown.
5. Click **New Application** (the docs call it **Create App**), enter `<PRODUCT_NAME>`, accept the terms and click **Create**.
6. On **General Information** note the **Application ID** (and the **Public Key**, only needed for interactions endpoints, which we do not use). Add an icon and description if you like.

### 2. Create the bot user and get the token

7. In the left sidebar open **Bot**.
8. Under **Token** click **Reset Token** and confirm (Discord may ask for 2FA). **Copy the token immediately**: it is shown once. Resetting later invalidates the old token.
9. Turn **Public Bot** off so only you can add the bot. (Each user will create their own application in the BYO model.)

### 3. Privileged Gateway Intents (still on the Bot page)

Discord lists three privileged intents under **Privileged Gateway Intents**. Turn on only what you need ("You may only toggle privileged intents that your bot requires"):

| Intent (portal label: verify on screen) | API name | Turn on? | Why |
|---|---|---|---|
| **Message Content Intent** | `MESSAGE_CONTENT` | **Yes, if the bot reads chat** | Without it the `content` and `embeds` of other people's messages arrive empty (you still get content in DMs, mentions and your own messages). The multichat bridge needs the text. |
| **Server Members Intent** | `GUILD_MEMBERS` | Off for the MVP | Only for join/leave events and member lists. Turn on later if we add role-based filtering or welcome automations. |
| **Presence Intent** | `GUILD_PRESENCES` | **No** | Online/offline and activity data. We do not need it. |

If you connect with an intent that is not enabled or approved, the gateway closes with code **4014**.

### 4. Choose permissions and build the invite

10. Open **Installation** in the sidebar. Under **Installation Contexts** keep **Guild Install** selected (User Install is not needed). Under **Install Link** choose **Discord Provided Link**. Under **Default Install Settings > Guild Install** add the scope `bot` (it includes `applications.commands`) and pick the permissions below.
11. Alternatively build the invite URL yourself (documented format; the portal may also show an OAuth2 URL Generator, verify on screen):

`https://discord.com/oauth2/authorize?client_id=<APPLICATION_ID>&scope=bot&permissions=274878024768`

**Computed permissions (minimal useful bot)**

| Permission | Why | Decimal |
|---|---|---|
| View Channels | see the channels | 1024 |
| Send Messages | go-live and relay posts | 2048 |
| Add Reactions | acknowledgements | 64 |
| Embed Links | rich go-live embeds | 16384 |
| Attach Files | clips and images | 32768 |
| Read Message History | needed when reading chat | 65536 |
| Send Messages in Threads | posting inside threads | 274877906944 |
| **Total** | | **274878024768** |

Optional additions (only when you enable moderation features): **Manage Messages** (+8192, total 274878032960) and **Moderate Members** for timeouts (+1099511627776). **Never** grant Administrator.

### 5. Invite the bot to your test server

12. Open the invite URL in a browser where you are logged in to Discord. You need the **Manage Server** permission on the target server (you do on your own).
13. Select your **test server**, click **Authorize**, solve the CAPTCHA.
14. In the server, make sure the bot's role can see and post in the test channels (channel permission overwrites can still block it).

Recommendation: always develop against a throwaway test server. A bot bug then spams only you.

### 6. OAuth2 client for `webhook.incoming` (D-059 target; prepare now, finalize after the broker exists)

This is a different credential from the bot token. The bot token (step 2) authenticates the bot user; the **OAuth2 client secret** authenticates the application in token exchanges, and Discord requires it for every OAuth2 token call ([OAuth2](https://docs.discord.com/developers/topics/oauth2), re-read 2026-10-19). It therefore lives only in the hosted broker.

15. In the same application open **OAuth2** in the sidebar (labels: verify on screen). The **Client ID** equals the Application ID. Under **Client Secret** click **Reset Secret** (2FA prompt), copy it once and store it in the password manager. Never put it in the desktop app, the repo or a chat.
16. **Redirects:** add exactly one entry, the hosted broker's callback URL. That URL is fixed by the broker design (DEN-244 artifact, N2) and **does not exist yet**: leave the list empty until then. Do not invent a hostname.
17. The scope `webhook.incoming` is requested at authorization time, not toggled in the portal. Once the redirect exists, the consent URL is `https://discord.com/oauth2/authorize?client_id=<APPLICATION_ID>&response_type=code&scope=webhook.incoming&redirect_uri=<BROKER_CALLBACK>` (URL-encode the redirect). The user picks a channel on Discord's page; the broker's token response carries `webhook.id`, `webhook.token` and `webhook.url`, "a new webhook is created each time".

## What to copy and where it goes

| Value | Secret? | Where it goes |
|---|---|---|
| **Bot token** | **Yes** | `SL_DISCORD_BOT_TOKEN` in `.env.local` (gitignored), or the OS keychain through Settings > Connections > Discord. Never in chat or the repo. |
| Application ID | No | `SL_DISCORD_APP_ID` (same places) |
| **OAuth2 Client Secret** (step 15) | **Yes** | Password manager now; the broker's secret store (`wrangler secret put`) once it exists. Never the desktop. |
| Public Key | No | Not needed now |
| Webhook URL | Yes (acts as a credential) | The StreamDen settings for the "post to Discord" action |

(Names are suggestions; P0 decides the real location.)

## Verification and test

```bash
read -rs DTOKEN
curl -s -H "Authorization: Bot $DTOKEN" https://discord.com/api/v10/users/@me   # bot user JSON, HTTP 200
# webhook test (HTTP 204 means success):
curl -s -o /dev/null -w '%{http_code}\n' -H 'Content-Type: application/json' \
  -d '{"content":"StreamDen webhook test"}' '<WEBHOOK_URL>'
```

For the gateway and intents check, use the app once it exists: connect and confirm message `content` is non-empty in your test channel.

## Common pitfalls

- **Close code 4014**: you requested an intent that is not toggled on the Bot page.
- **Empty `content`**: the Message Content Intent is off.
- **Bot can't post**: missing View Channels or Send Messages in that channel, or a channel overwrite denies it.
- **Token reset** invalidates the old token and drops running gateway sessions.
- **IDENTIFY limit**: 1,000 per 24 hours. Reconnect loops can burn it.
- **Webhook URL leaked** (for example in a screenshot): delete the webhook in the server settings and create a new one.
- **`webhook.incoming` OAuth** and any OAuth token call need a `client_secret`, so the browser consent waits for the hosted broker and the pasted webhook URL is the interim path (D-059).

## Policy notes

- Discord Developer Terms and Policy apply. Treat the bot token like a password.
- `MESSAGE_CONTENT` is privileged: Discord requires review once an app reaches 10,000 users, and verification at 100+ servers. Per-user BYO bots stay far below that.
- Read only channels the user configured, and tell users what the bot stores.

## Sources

- Getting started (create app, token, installation): <https://docs.discord.com/developers/quick-start/getting-started>
- Gateway intents (privileged): <https://docs.discord.com/developers/events/gateway>
- OAuth2 and bot authorization flow: <https://docs.discord.com/developers/topics/oauth2>
- Permissions: <https://docs.discord.com/developers/topics/permissions>
- Rate limits: <https://docs.discord.com/developers/topics/rate-limits>
- Developer Portal: <https://discord.com/developers/applications>
