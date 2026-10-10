#!/usr/bin/env bash
set -euo pipefail

# Parse the complete function before pulling: a pull may update this file too.
main() {
    if [[ ${1:-} == --help || ${1:-} == -h ]]; then
        printf '%s\n' 'Usage: bash update.sh' \
            'Pull the current branch, install npm dependencies, and build Flagship.' \
            'Run as the deployment user. Running services restart after a successful build (sudo required).' \
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
    for flagship_command in git node npm cargo flock; do
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
    cargo build --release --locked --jobs "$flagship_jobs"
    printf '%s\n' 'Update and build complete.'
    if command -v systemctl >/dev/null 2>&1; then
        if systemctl is-active --quiet flagship.service; then
            # Directus is part of Flagship: PartOf propagates this restart,
            # and Wants starts it with the website if it was stopped.
            printf '%s\n' 'Restarting Flagship and its Directus service...'
            sudo systemctl restart flagship.service
        elif systemctl is-active --quiet flagship-directus.service; then
            printf '%s\n' 'Restarting Directus; Flagship remains stopped...'
            sudo systemctl restart flagship-directus.service
        else
            printf '%s\n' 'Services are stopped or not installed; nothing to restart.'
        fi
    else
        printf '%s\n' 'systemctl is unavailable; nothing to restart.'
    fi
}

main "$@"
