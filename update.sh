#!/usr/bin/env bash
set -euo pipefail

# Parse the complete function before pulling: a pull may update this file too.
main() {
    if [[ ${1:-} == --help || ${1:-} == -h ]]; then
        printf '%s\n' 'Usage: bash update.sh' \
            'Pull the current branch, install npm dependencies, and build Flagship.' \
            'Run as the deployment user. After building, update project services and validate/install/reload Caddy (sudo required).' \
            'CARGO_BUILD_JOBS defaults to 2; set it to change build parallelism.'
        return
    fi
    if (( $# != 0 )); then
        printf '%s\n' 'Usage: bash update.sh' >&2
        return 1
    fi
    if (( EUID == 0 )); then
        printf '%s\n' 'Run this script as your deployment user (hazem), without sudo.' >&2
        return 1
    fi

    local flagship_root flagship_jobs
    flagship_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
    cd "$flagship_root"
    if ! command -v cargo >/dev/null 2>&1 && [[ -f "$HOME/.cargo/env" ]]; then
        source "$HOME/.cargo/env"
    fi
    for flagship_command in git node npm cargo flock systemctl systemd-analyze sudo install curl caddy; do
        if ! command -v "$flagship_command" >/dev/null 2>&1; then
            printf 'Missing prerequisite: %s\n' "$flagship_command" >&2
            return 1
        fi
    done
    node -e 'const [major, minor] = process.versions.node.split(".").map(Number); if (major !== 22 || minor < 12) { console.error("Use Node.js 22.12 or newer within Node 22 for Directus."); process.exit(1); }'
    flagship_jobs=${CARGO_BUILD_JOBS:-2}
    if [[ ! "$flagship_jobs" =~ ^[1-9][0-9]*$ ]]; then
        printf '%s\n' 'CARGO_BUILD_JOBS must be a positive integer.' >&2
        return 1
    fi

    # Keep simultaneous deployments from modifying the same build directories.
    local flagship_git_dir
    flagship_git_dir=$(git rev-parse --absolute-git-dir)
    exec 9>"$flagship_git_dir/flagship-update.lock"
    if ! flock -n 9; then
        printf '%s\n' 'Another update is already running.' >&2
        return 1
    fi
    if [[ -n $(git status --porcelain --untracked-files=no) ]]; then
        printf '%s\n' 'Commit or stash tracked server changes before updating. Your ignored .env is unaffected.' >&2
        return 1
    fi

    printf '%s\n' 'Pulling the current branch...'
    git pull --ff-only

    printf '%s\n' 'Installing and building the website...'
    (cd website && npm ci --include=dev --no-audit --no-fund && npm run build)

    printf '%s\n' 'Installing Directus dependencies...'
    (cd deploy/directus && npm ci --omit=dev --no-audit --no-fund)

    printf '%s\n' 'Building the Rust release binary...'
    cargo build --release --bin Flagship --locked --jobs "$flagship_jobs"
    # Reject invalid proxy configuration before restarting any project services.
    printf '%s\n' 'Validating the Caddy configuration...'
    sudo caddy validate --config "$flagship_root/deploy/Caddyfile" --adapter caddyfile
    printf '%s\n' 'Updating the retailer browser and proxy services...'
    bash deploy/install-retailer-services.sh
    printf '%s\n' 'Update and build complete.'
    printf '%s\n' 'Verifying the new systemd service files...'
    # Verify before replacing installed units or restarting any services.
    sudo systemd-analyze verify "$flagship_root/deploy/flagship.service" "$flagship_root/deploy/flagship-directus.service"
    printf '%s\n' 'Installing service files and reloading systemd...'
    sudo install -m 644 "$flagship_root/deploy/flagship.service" "$flagship_root/deploy/flagship-directus.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable flagship.service
    # restart also starts an inactive service. One transaction handles both
    # units and their PartOf/Wants relationship without sequential restarts.
    printf '%s\n' 'Starting/restarting Flagship and Directus...'
    sudo systemctl restart flagship.service flagship-directus.service

    local flagship_directus_port
    flagship_directus_port=$(node --input-type=module -e '
        import { readFileSync } from "node:fs";
        import { parseEnv } from "node:util";
        const settings = { ...parseEnv(readFileSync(".env", "utf8")), ...process.env };
        const port = Number(settings.DIRECTUS_PORT || "8055");
        if (!Number.isInteger(port) || port < 1 || port > 65535) process.exit(1);
        console.log(port);
    ')
    wait_ready flagship.service http://127.0.0.1:3000/health/ready
    wait_ready flagship-directus.service "http://127.0.0.1:$flagship_directus_port/server/ping"
    printf '%s\n' 'Installing the Caddy configuration and reloading the proxy...'
    sudo install -m 644 "$flagship_root/deploy/Caddyfile" /etc/caddy/Caddyfile
    # Reload gracefully when running; also start Caddy if it is inactive.
    sudo systemctl reload-or-restart caddy.service
    printf '%s\n' 'Deployment complete. Flagship and Directus are ready; Caddy configuration applied.'
}

wait_ready() {
    local flagship_unit=$1 flagship_url=$2 flagship_attempt
    printf 'Waiting for %s...\n' "$flagship_unit"
    for (( flagship_attempt=0; flagship_attempt<120; flagship_attempt++ )); do
        if curl --noproxy '*' -fsS --max-time 2 -o /dev/null \
            -H 'X-Real-IP: 127.0.0.1' "$flagship_url" 2>/dev/null; then
            return
        fi
        if systemctl is-failed --quiet "$flagship_unit"; then
            break
        fi
        sleep 1
    done
    printf '%s did not become ready. Inspect: sudo journalctl -u %s -n 80 --no-pager\n' \
        "$flagship_unit" "$flagship_unit" >&2
    return 1
}

main "$@"
