#!/usr/bin/env bash
# Builds a release build, packages it for a connected iPhone, installs it and
# launches it. The loop run by hand whenever an iOS change needs the real
# device (the simulator does not show ProMotion, the device's frame rate, or
# the GPU's behaviour).
#
# Usage: scripts/ios-device.sh <app-dir> [--screenshot <path>] [--console]
#
# The signing identity and the provisioning profile are per machine: set
# NSH_SIGN (an identity, `security find-identity -v -p codesigning` lists
# them) and NSH_PROFILE (a .mobileprovision path). The device is picked from
# NSH_DEVICE (a name or a udid), else the first one `devicectl` lists.
#
# `--console` streams the app's log to stdout (WARN/ERROR from `log`, plus
# anything the app prints) — the usual way to read a HUD's diagnostic lines.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
nsh="$root/target/release/nsh"

app_dir="${1:-}"
if [ -z "$app_dir" ]; then
    echo "usage: $0 <app-dir> [--screenshot <path>] [--console]" >&2
    exit 2
fi
shift

screenshot=""
console=0
while [ $# -gt 0 ]; do
    case "$1" in
        --screenshot) screenshot="${2:?--screenshot needs a path}"; shift 2 ;;
        --console) console=1; shift ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

: "${NSH_SIGN:?set NSH_SIGN to a code signing identity (\`security find-identity -v -p codesigning\`)}"
: "${NSH_PROFILE:?set NSH_PROFILE to a .mobileprovision path}"

if [ ! -x "$nsh" ]; then
    echo "reading the release CLI ($nsh missing): cargo build --release -p nsh-cli" >&2
    (cd "$root" && cargo build --release -p nsh-cli)
fi

device="${NSH_DEVICE:-}"
if [ -z "$device" ]; then
    # The table's Identifier column carries the udid as "… (UDID)"; the state
    # is "available (paired)" for a reachable device.
    device="$(xcrun devicectl list devices 2>/dev/null \
        | awk '/available/ && /iPhone/ {print $3; exit}' \
        | sed 's/ *(UDID)$//')"
fi
if [ -z "$device" ]; then
    echo "no device: connect an iPhone or set NSH_DEVICE" >&2
    exit 1
fi

app_dir="$(cd "$app_dir" && pwd)"
name="$(basename "$app_dir")"

echo "packaging $name for $device"
"$nsh" package "$app_dir" --target ios --sign "$NSH_SIGN" --profile "$NSH_PROFILE"

# `nsh package` names both the .ipa and the .app after the app's display name
# (`CFBundleName`), not after the directory.
out="$app_dir/out"
ipa="$(find "$out" -maxdepth 1 -name '*.ipa' -print -quit 2>/dev/null || true)"
app="$(find "$out" -maxdepth 1 -name '*.app' -print -quit 2>/dev/null || true)"
if [ -z "$ipa" ] || [ -z "$app" ]; then
    echo "no .ipa / .app in $out" >&2
    exit 1
fi

id="$(plutil -extract CFBundleIdentifier raw "$app/Info.plist" 2>/dev/null || true)"
if [ -z "$id" ]; then
    echo "no CFBundleIdentifier in $app/Info.plist" >&2
    exit 1
fi

echo "installing $id"
xcrun devicectl device install app --device "$device" "$ipa" >/dev/null

echo "launching $id"
if [ "$console" = 1 ]; then
    # Only one launch can hold the console: stop a previous one first.
    xcrun devicectl device process terminate --device "$device" "$id" >/dev/null 2>&1 || true
    exec xcrun devicectl device process launch --device "$device" --console "$id"
fi

# A locked device refuses to launch (`BSErrorCodeDescription = Locked`).
if ! xcrun devicectl device process launch --device "$device" "$id" 2>&1 | tail -1; then
    echo "launch failed: unlock the device and retry" >&2
fi

if [ -n "$screenshot" ]; then
    sleep 4
    xcrun devicectl device capture screenshot --device "$device" --destination "$screenshot"
    echo "screenshot: $screenshot"
fi
