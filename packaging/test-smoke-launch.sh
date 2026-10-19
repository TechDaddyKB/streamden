#!/usr/bin/env bash
# Exercise the real Xvfb/D-Bus probe with controlled desktop executables.
set -euo pipefail
ulimit -c 0
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
fixture_dir=$(mktemp -d)
trap 'rm -rf "$fixture_dir"' EXIT
export PATH="$fixture_dir:$PATH"

cat > "$fixture_dir/streamden" <<'EOF'
#!/usr/bin/env bash
echo 'fuse: device not found'
echo "fuse init failed: Can't mount path /tmp/cache/doc"
echo 'Gtk-WARNING: cannot open display: portal helper'
exec sleep 60
EOF
chmod +x "$fixture_dir/streamden"
bash "$script_dir/smoke-launch.sh"
echo 'PASS: surviving desktop tolerates unrelated session diagnostics'

for outcome in crash clean-exit; do
  cat > "$fixture_dir/streamden" <<'EOF'
#!/usr/bin/env bash
if [[ $SMOKE_TEST_OUTCOME == crash ]]; then
  kill -SEGV "$$"
fi
exit 0
EOF
  if SMOKE_TEST_OUTCOME="$outcome" bash "$script_dir/smoke-launch.sh" > "$fixture_dir/result.log" 2>&1; then
    cat "$fixture_dir/result.log"
    echo "FAIL: probe accepted desktop $outcome" >&2
    exit 1
  fi
  cat "$fixture_dir/result.log"
  # Ensure it failed for the desktop exit, rather than broken probe infrastructure.
  grep -Fq 'Desktop exited before the 15-second launch probe:' "$fixture_dir/result.log"
  echo "PASS: probe rejects desktop $outcome"
done
