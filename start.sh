#!/bin/sh
set -eu

flagship_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -x "$flagship_root/state/node22/bin/node" ]; then
    flagship_node="$flagship_root/state/node22/bin/node"
elif command -v node >/dev/null 2>&1; then
    flagship_node=node
else
    echo "Install Node.js 22 LTS with npm first." >&2
    exit 1
fi
cd "$flagship_root"
exec "$flagship_node" "$flagship_root/deploy/local.mjs" "$@"
