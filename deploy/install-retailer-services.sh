#!/usr/bin/env bash
set -euo pipefail
if (( EUID == 0 )); then
    printf '%s\n' 'Run as hazem, without sudo; the script uses sudo for systemd files.' >&2
    exit 1
fi
[[ $(id -un) == hazem ]] || { printf '%s\n' 'These deployment units use the hazem account.' >&2; exit 1; }
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
runtime_dir="$HOME/.local/share/flagship-scraping/bin"
for command in warp-cli xvfb-run curl systemctl systemd-analyze; do
    command -v "$command" >/dev/null || { printf 'Missing prerequisite: %s\n' "$command" >&2; exit 1; }
done
mkdir -p "$runtime_dir"
install -m 755 "$source_dir/retailer-browser.sh" "$source_dir/retailer-warp.sh" "$runtime_dir/"
sudo systemd-analyze verify "$source_dir/flagship.service" "$source_dir/flagship-browser.service" "$source_dir/flagship-warp.service"
sudo rm -f -- /etc/flagship/retailers.env
sudo rm -f -- /etc/systemd/system/flagship.service.d/retailers.conf
sudo install -m 644 "$source_dir/flagship.service" "$source_dir/flagship-browser.service" "$source_dir/flagship-warp.service" /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable flagship-warp.service flagship-browser.service
sudo systemctl restart flagship-warp.service flagship-browser.service
printf '%s\n' 'Retailer services installed. Restart Flagship after validating the new build.'
