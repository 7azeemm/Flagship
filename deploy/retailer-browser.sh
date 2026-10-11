#!/usr/bin/env bash
set -euo pipefail

profile="$HOME/.local/share/flagship-browser"
shopt -s nullglob
candidates=("$HOME"/.cache/ms-playwright/chromium-*/chrome-linux64/chrome)
if (( ${#candidates[@]} == 0 )); then
    printf '%s\n' 'Install Playwright Chromium first.' >&2
    exit 1
fi
browser=$(printf '%s\n' "${candidates[@]}" | sort -V | tail -n 1)
[[ -x "$browser" ]] || { printf 'Chromium is not executable: %s\n' "$browser" >&2; exit 1; }
mkdir -p "$profile"
chmod 700 "$profile"
# The CDP endpoint and SOCKS proxy are local only. Run a normal headed browser;
# Playwright attaches afterwards rather than supplying browser launch flags.
exec /usr/bin/xvfb-run -a -s '-screen 0 1280x900x24' "$browser" \
    --user-data-dir="$profile" \
    --remote-debugging-address=127.0.0.1 --remote-debugging-port=9224 \
    --proxy-server=socks5://127.0.0.1:40000 \
    --disable-dev-shm-usage --no-sandbox --no-first-run --no-default-browser-check about:blank
