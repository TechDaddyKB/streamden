#!/usr/bin/env bash
# Seed an ISOLATED OBS 32.x config tree for spike S-2. Never points at ~/.config/obs-studio.
# Usage: seed-obs-config.sh <root> <server-port> [ws-port]
#   <root>/config/obs-studio/...  (use with XDG_CONFIG_HOME=<root>/config)
set -euo pipefail
ROOT=${1:?root dir, e.g. /tmp/den-s2-obs}
PORT=${2:?spike server port}
WSPORT=${3:-4466}
case "$ROOT" in /tmp/*) ;; *) echo "refusing: root must be under /tmp" >&2; exit 1 ;; esac

OBS="$ROOT/config/obs-studio"
mkdir -p "$OBS/basic/profiles/DenS2" "$OBS/basic/scenes" "$OBS/plugin_config/obs-websocket"

# global.ini (AppConfig): LastVersion set => no auto-config wizard; Pre31Migrated => no settings migration.
# 537001986 = MAKE_SEMANTIC_VERSION(32, 2, 2)
cat > "$OBS/global.ini" <<EOF
[General]
LastVersion=537001986
Pre31Migrated=true
EOF

# user.ini (UserConfig, OBS >= 31). Custom browser docks live in [BasicWindow] ExtraBrowserDocks as JSON
# (OBSBasic::LoadExtraBrowserDocks / SaveExtraBrowserDocks in frontend/widgets/OBSBasic_Browser.cpp).
cat > "$OBS/user.ini" <<EOF
[General]
FirstRun=true

[Basic]
Profile=DenS2
ProfileDir=DenS2
SceneCollection=DenS2
SceneCollectionFile=DenS2

[BasicWindow]
ExtraBrowserDocks=[{"title":"StreamDen S-2 Chat","url":"http://127.0.0.1:${PORT}/dock/chat","uuid":"5d2e0c1a7b6f4e1c9a3d8f2b6c4e1a90"}]
EOF

cat > "$OBS/basic/profiles/DenS2/basic.ini" <<EOF
[General]
Name=DenS2

[Video]
BaseCX=1920
BaseCY=1080
OutputCX=1920
OutputCY=1080
FPSType=0
FPSCommon=30
EOF

# obs-websocket 5.x config (plugin_config/obs-websocket/config.json). Random password, kept only in /tmp.
if [ ! -f "$OBS/plugin_config/obs-websocket/config.json" ]; then
  PW=$(head -c 24 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24)
  cat > "$OBS/plugin_config/obs-websocket/config.json" <<EOF
{"alerts_enabled":false,"auth_required":true,"first_load":false,"server_enabled":true,"server_password":"$PW","server_port":$WSPORT}
EOF
  chmod 600 "$OBS/plugin_config/obs-websocket/config.json"
fi
echo "seeded $OBS (dock -> http://127.0.0.1:${PORT}/dock/chat, websocket port $WSPORT)"
