#!/usr/bin/env bash
# Launch an orchestrator-managed agent on a StreamDen board card, in its own worktree.
#
#   scripts/dispatch.sh DEN-<n> <claude|codex|grok> [work|review] [slug]
#
# work   (default) — create/reuse ../den-wt/DEN-<n> (or ../den-wt/DEN-<n>-<vendor> if another vendor owns it)
#                    on branch agent/<vendor>/DEN-<n>-<slug>, then start the agent.
# review           — reuse the existing ../den-wt/DEN-<n> worktree (the author's branch) and start a reviewer.
#
# The agent follows AGENTS.md + docs/agents/board.md: it claims the card itself, works only in this worktree,
# appends its handoff to the card and releases it. Nothing is pushed.
set -euo pipefail

usage() { sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
[[ $# -ge 2 ]] || usage
card="$1"; vendor="$2"; mode="${3:-work}"; slug="${4:-task}"
[[ "$card" =~ ^DEN-([0-9]+)$ ]] || { echo "card must look like DEN-12" >&2; exit 2; }
n="${BASH_REMATCH[1]}"
case "$vendor" in
  claude) wrapper=clx; model="${DEN_MODEL:-claude-opus-5-5}" ;;
  codex)  wrapper=cdx; model="${DEN_MODEL:-gpt-6.1-sol}" ;;
  grok)   wrapper=cgx; model="${DEN_MODEL:-grok-4.7}" ;;
  *) echo "vendor must be claude, codex or grok" >&2; exit 2 ;;
esac
[[ "$mode" == work || "$mode" == review ]] || usage
[[ "$slug" =~ ^[a-z0-9-]+$ ]] || { echo "slug must be lowercase letters, digits, hyphens" >&2; exit 2; }

# Shells started from desktop AppImages (e.g. Enjoy) leak library/plugin paths that break git, curl and GUI apps.
unset LD_LIBRARY_PATH GST_PLUGIN_SYSTEM_PATH GST_PLUGIN_SYSTEM_PATH_1_0 GST_PLUGIN_PATH APPIMAGE APPDIR OWD PYTHONHOME PYTHONPATH

repo="$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)"
wt="$(dirname "$repo")/den-wt/$card"
username="$vendor/$model"
today="$(date +%F)"

if [[ "$mode" == work ]]; then
  # Bake-off style cards are worked by several vendors: give each vendor its own worktree.
  if [[ -d "$wt" ]] && [[ "$(git -C "$wt" branch --show-current)" != agent/"$vendor"/* ]]; then
    wt="$wt-$vendor"
  fi
  if [[ -d "$wt" ]]; then
    echo "Reusing worktree $wt ($(git -C "$wt" branch --show-current))"
  else
    git -C "$repo" fetch --quiet origin main || true
    base="origin/main"; git -C "$repo" rev-parse --verify --quiet "$base" >/dev/null || base="main"
    git -C "$repo" worktree add "$wt" -b "agent/$vendor/$card-$slug" "$base"
  fi
  prompt="You are $username working on StreamDen board card $card (card #$n, project slug 'streamden').
Follow AGENTS.md and docs/agents/board.md exactly:
1. project_summary(streamden), then project_card_get(card $n). Read its detail and history; respect depends_on/ready.
2. Claim it: project_card_claim(slug streamden, card $n, role = plan for kind:spec cards, else code, worktree_path=$wt, username=$username, client_request_id=$card-$mode-$today), then move it to your lane.
3. Work ONLY in this worktree ($wt) and only on the card's Scope (paths). Run its Done-when commands and keep evidence.
4. Commit per AGENTS.md (D-037): Conventional Commit subject ending in ($card), a body with what/why and the checks you ran, then exactly one AI-Assisted-By trailer for your engine. No co-author lines. Do not push.
5. Append your handoff (tasks/_TEMPLATE.md §B) to the card detail, then project_card_release with a one-line note (advance only if every Done-when passes; otherwise resolution handoff or blocked)."
else
  [[ -d "$wt" ]] || { echo "No worktree $wt to review" >&2; exit 1; }
  branch="$(git -C "$wt" branch --show-current)"
  prompt="You are $username reviewing StreamDen board card $card (card #$n, project slug 'streamden').
1. project_summary(streamden), project_card_get(card $n). Claim it with role review (worktree_path=$wt, username=$username, client_request_id=$card-review-$today).
2. Review branch $branch against origin/main (git diff origin/main...HEAD) and the card's Goal/Checks/Done-when, following the 'Code review rules' in AGENTS.md. Run the Done-when commands yourself. Do not edit code.
3. Append '### Review — $today — $username' to the card detail: findings first, by severity, each with file:line, failure scenario and suggested fix.
4. Release: no resolution (advance to Verifying) if nothing blocking; otherwise project_card_release with column coding and a one-line note."
fi

cd "$wt"
echo "Launching $wrapper in $wt ($mode $card)…"
exec "$wrapper" "$prompt"
