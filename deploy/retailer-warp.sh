#!/usr/bin/env bash
set -euo pipefail

# Only proxy mode: the backend API, SSH and unrelated HTTP clients keep their route.
warp-cli mode proxy
warp-cli proxy port 40000
warp-cli connect
trap 'warp-cli disconnect >/dev/null 2>&1 || true; exit 0' TERM INT
failures=0
while true; do
    if curl --noproxy '' --silent --fail --max-time 10 --proxy socks5h://127.0.0.1:40000 \
        https://www.cloudflare.com/cdn-cgi/trace | grep -q '^warp=on'; then
        failures=0
    else
        failures=$((failures + 1))
        if (( failures >= 3 )); then
            printf '%s\n' 'Retailer WARP proxy failed three health checks; reconnecting via systemd restart.' >&2
            exit 1
        fi
    fi
    sleep 15 &
    wait $!
done
