#!/usr/bin/env bash
# Launch an orchestrator-managed agent on a StreamDen board card, in its own worktree.
#
#   scripts/dispatch.sh DEN-<n> <claude|codex|grok> [work|review] [slug]
#
# work   (default) — create/reuse ../den-wt/DEN-<n> (or ../den-wt/DEN-<n>-<vendor> if another vendor owns it)
#                    on branch agent/<vendor>/DEN-<n>-<slug>, then start the agent.
# review           — reuse the existing ../den-wt/DEN-<n> worktree (the author's branch) and start a reviewer.
#
# Codex runs GPT-6.1 Sol at medium reasoning effort by default (D-044); override with DEN_EFFORT=<low|medium|high>.
# DEN_DRY_RUN=1 prints the launch command (and the prompt) instead of starting the agent.
#
# The agent follows AGENTS.md + docs/agents/board.md: it claims the card itself, works only in this worktree,
# appends its handoff to the card and releases it. Nothing is pushed.
set -euo pipefail

usage() { sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
[[ $# -ge 2 ]] || usage
card="$1"; vendor="$2"; mode="${3:-work}"; slug="${4:-task}"
[[ "$card" =~ ^DEN-([0-9]+)$ ]] || { echo "card must look like DEN-12" >&2; exit 2; }
n="${BASH_REMATCH[1]}"
wrapper_args=()
case "$vendor" in
  claude) wrapper=clx; model="${DEN_MODEL:-claude-opus-5-5}" ;;
  codex)  wrapper=cdx; model="${DEN_MODEL:-gpt-6.1-sol}"
          wrapper_args=(-m "$model" -c "model_reasoning_effort=\"${DEN_EFFORT:-medium}\"") ;;
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
# Generate once per dispatch; retries use the same ID embedded in the prompt.
claim_request_id="$card-$vendor-$mode-$(date -u +%Y%m%dT%H%M%SZ)-$$-$RANDOM"

dry() { [[ "${DEN_DRY_RUN:-}" == 1 ]]; }

if [[ "$mode" == work ]]; then
  # Bake-off style cards are worked by several vendors: give each vendor its own worktree.
  if [[ -d "$wt" ]] && [[ "$(git -C "$wt" branch --show-current)" != agent/"$vendor"/* ]]; then
    wt="$wt-$vendor"
  fi
  if dry; then
    :
  elif [[ -d "$wt" ]]; then
    echo "Reusing worktree $wt ($(git -C "$wt" branch --show-current))"
  else
    git -C "$repo" fetch --quiet origin main || true
    base="origin/main"; git -C "$repo" rev-parse --verify --quiet "$base" >/dev/null || base="main"
    git -C "$repo" worktree add "$wt" -b "agent/$vendor/$card-$slug" "$base"
  fi
  if dry; then
    :
  elif [[ -f "$wt/package.json" && -f "$wt/pnpm-lock.yaml" ]]; then
    echo "Installing frozen dependencies in $wt"
    if ! (cd "$wt" && pnpm install --frozen-lockfile --ignore-scripts); then
      echo "Dependency install failed in $wt; agent not launched. Run pnpm install --frozen-lockfile --ignore-scripts there before retrying." >&2
      exit 1
    fi
  else
    echo "Dependency install skipped in $wt: package.json and pnpm-lock.yaml are both required."
  fi
  prompt="You are $username working on StreamDen board card $card (card #$n, project slug 'streamden').
Follow AGENTS.md and docs/agents/board.md exactly:
1. project_summary(streamden), then project_card_get(card $n). Read its detail and history; respect depends_on/ready.
2. Claim it: project_card_claim(slug streamden, card $n, role = plan for kind:spec cards, else code, worktree_path=$wt, username=$username, client_request_id=$claim_request_id), then move it to your lane only after confirming the claim is yours.
3. Work ONLY in this worktree ($wt) and only on the card's Scope (paths). Run its Done-when commands and keep evidence.
4. Commit per AGENTS.md (D-037): Conventional Commit subject ending in ($card), a body with what/why and the checks you ran, then exactly one AI-Assisted-By trailer for your engine. No co-author lines. Do not push.
5. Append your handoff (tasks/_TEMPLATE.md §B) to the card detail, then project_card_release with a one-line note (advance only if every Done-when passes; otherwise resolution handoff or blocked)."
else
  dry || [[ -d "$wt" ]] || { echo "No worktree $wt to review" >&2; exit 1; }
  branch="$(git -C "$wt" branch --show-current 2>/dev/null || true)"
  prompt="You are $username reviewing StreamDen board card $card (card #$n, project slug 'streamden').
1. project_summary(streamden), project_card_get(card $n). Claim it with role review (worktree_path=$wt, username=$username, client_request_id=$claim_request_id).
2. Review branch $branch against origin/main (git diff origin/main...HEAD) and the card's Goal/Checks/Done-when, following the 'Code review rules' in AGENTS.md. Run the Done-when commands yourself. Do not edit code.
3. Append '### Review — $today — $username' to the card detail: findings first, by severity, each with file:line, failure scenario and suggested fix.
4. Release: no resolution (advance to Verifying) if nothing blocking; otherwise project_card_release with column coding and a one-line note."
fi

prompt="$prompt
Claim safety (including shared bake-off cards): confirm the live claim with project_card_get using worktree_path=$wt and username=$username; require claim.yours=true before working, moving or releasing the card. A replayed claimed:true response alone is insufficient. If another vendor/session holds it, report the holder and stop this dispatch; never release or move another vendor's live claim. If the card is unheld and the claim is not yours, report and stop; do not re-dispatch with the same ID. For transport retries within this dispatch only, reuse this exact client_request_id: $claim_request_id"

if dry; then
  printf 'Would launch in %s: %s' "$wt" "$wrapper"; (( ${#wrapper_args[@]} )) && printf ' %q' "${wrapper_args[@]}"; printf ' <prompt>\n---\n%s\n' "$prompt"
  exit 0
fi
cd "$wt"
echo "Launching $wrapper in $wt ($mode $card)…"
exec "$wrapper" "${wrapper_args[@]}" "$prompt"
