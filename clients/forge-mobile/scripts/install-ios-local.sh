#!/bin/sh

set -eu

usage() {
  cat <<'EOF'
Build, verify, install, and launch Forge on a personally owned iPhone.

Usage:
  scripts/install-ios-local.sh --team TEAM_ID --device DEVICE_ID --bundle-id BUNDLE_ID [options]

Required:
  --team TEAM_ID       Apple development team ID shown by Xcode
  --device DEVICE_ID   CoreDevice identifier, hardware UDID, or unique device name
  --bundle-id ID       Bundle ID unique to your Apple account, for example com.example.forge.dev

Options:
  --derived-data PATH  Xcode DerivedData directory (default: a temporary directory)
  --skip-prebuild      Reuse the existing generated apps/mobile/ios project
  --no-launch          Install but do not launch Forge
  --dry-run            Validate inputs and print commands without changing files or the device
  -h, --help           Show this help

Requirements:
  macOS, Xcode, Node.js 24 or newer, pnpm 11, CocoaPods, and an unlocked,
  paired iPhone with Developer Mode enabled. Add a free Apple account under
  Xcode > Settings > Accounts before running this command.

This creates a locally signed development build. It does not create a
redistributable IPA or upload anything to TestFlight.
EOF
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

quote() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

print_command() {
  first=1
  for argument in "$@"; do
    if [ "$first" -eq 0 ]; then
      printf ' '
    fi
    quote "$argument"
    first=0
  done
  printf '\n'
}

run() {
  if [ "$dry_run" -eq 1 ]; then
    print_command "$@"
  else
    "$@"
  fi
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

team_id=
device_id=
bundle_id=
derived_data=
skip_prebuild=0
launch=1
dry_run=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --team)
      [ "$#" -ge 2 ] || fail "--team requires a value"
      team_id=$2
      shift 2
      ;;
    --device)
      [ "$#" -ge 2 ] || fail "--device requires a value"
      device_id=$2
      shift 2
      ;;
    --bundle-id)
      [ "$#" -ge 2 ] || fail "--bundle-id requires a value"
      bundle_id=$2
      shift 2
      ;;
    --derived-data)
      [ "$#" -ge 2 ] || fail "--derived-data requires a value"
      derived_data=$2
      shift 2
      ;;
    --skip-prebuild)
      skip_prebuild=1
      shift
      ;;
    --no-launch)
      launch=0
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --)
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

[ "$(uname -s)" = "Darwin" ] || fail "this installer requires macOS"
[ -n "$team_id" ] || fail "pass --team TEAM_ID"
[ -n "$device_id" ] || fail "pass --device DEVICE_ID"
[ -n "$bundle_id" ] || fail "pass --bundle-id BUNDLE_ID"

case "$team_id" in
  *[!A-Za-z0-9]*) fail "--team must contain only letters and numbers" ;;
esac
case "$bundle_id" in
  *.*) ;;
  *) fail "--bundle-id must be a reverse-DNS identifier" ;;
esac
case "$bundle_id" in
  *[!A-Za-z0-9.-]*|.*|*.|*..*) fail "--bundle-id contains an invalid component" ;;
esac

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
workspace_dir=$(CDPATH='' cd -- "$script_dir/.." && pwd)
mobile_dir="$workspace_dir/apps/mobile"
ios_dir="$mobile_dir/ios"

require_command node
require_command pnpm
require_command pod
require_command xcodebuild
require_command xcrun
require_command codesign
require_command security
require_command plutil

node_major=$(node -p 'Number(process.versions.node.split(".")[0])')
[ "$node_major" -ge 24 ] || fail "Node.js 24 or newer is required"

pnpm_major=$(pnpm --version | awk -F. '{ print $1 }')
[ "$pnpm_major" -eq 11 ] || fail "pnpm 11 is required"

if [ "$dry_run" -eq 0 ]; then
  xcodebuild -checkFirstLaunchStatus >/dev/null 2>&1 || fail "finish Xcode first-launch setup"
  if ! security find-identity -v -p codesigning | grep -Eq 'Apple Development|iPhone Developer'; then
    printf '%s\n' "warning: no Apple Development identity is present yet; Xcode will try to create one" >&2
  fi
fi

device_json_dir=$(mktemp -d "${TMPDIR:-/tmp}/forge-device.XXXXXX")
device_json="$device_json_dir/devices.json"
resolved_udid_file="$device_json_dir/udid"
xcrun devicectl list devices --json-output "$device_json" >/dev/null
DEVICE_ID=$device_id /usr/bin/python3 - "$device_json" "$resolved_udid_file" <<'PY'
import json
import os
import sys

requested = os.environ["DEVICE_ID"]
with open(sys.argv[1], encoding="utf-8") as handle:
    devices = json.load(handle).get("result", {}).get("devices", [])

matches = []
for device in devices:
    properties = device.get("deviceProperties", {})
    hardware = device.get("hardwareProperties", {})
    identifiers = {
        str(device.get("identifier", "")),
        str(hardware.get("udid", "")),
        str(properties.get("name", "")),
    }
    if requested in identifiers and hardware.get("platform") == "iOS":
        matches.append(device)

if len(matches) != 1:
    sys.exit(f"device {requested!r} did not identify exactly one paired iPhone")

device = matches[0]
if device.get("connectionProperties", {}).get("tunnelState") != "connected":
    sys.exit("the selected iPhone is not connected")
if device.get("connectionProperties", {}).get("pairingState") != "paired":
    sys.exit("the selected iPhone is not paired")
if device.get("deviceProperties", {}).get("developerModeStatus") != "enabled":
    sys.exit("Developer Mode is not enabled on the selected iPhone")

with open(sys.argv[2], "w", encoding="utf-8") as handle:
    handle.write(device.get("hardwareProperties", {}).get("udid", ""))
PY
xcode_device_id=$(cat "$resolved_udid_file")
[ -n "$xcode_device_id" ] || fail "the selected iPhone did not report a hardware UDID"

if [ -z "$derived_data" ]; then
  if [ "$dry_run" -eq 1 ]; then
    derived_data="${TMPDIR:-/tmp}/forge-ios-derived-data"
  else
    derived_data=$(mktemp -d "${TMPDIR:-/tmp}/forge-ios.XXXXXX")
  fi
fi

printf 'Forge iOS local install\n'
printf '  team:       %s\n' "$team_id"
printf '  device:     %s (%s)\n' "$device_id" "$xcode_device_id"
printf '  bundle ID:  %s\n' "$bundle_id"
printf '  build data: %s\n' "$derived_data"

if [ "$skip_prebuild" -eq 0 ]; then
  run pnpm --dir "$workspace_dir" prebuild:ios
fi

if [ "$dry_run" -eq 0 ]; then
  [ -d "$ios_dir" ] || fail "generated iOS project not found; rerun without --skip-prebuild"
fi

run xcodebuild \
  -workspace "$ios_dir/Forge.xcworkspace" \
  -scheme Forge \
  -configuration Release \
  -destination "platform=iOS,id=$xcode_device_id" \
  -derivedDataPath "$derived_data" \
  DEVELOPMENT_TEAM="$team_id" \
  PRODUCT_BUNDLE_IDENTIFIER="$bundle_id" \
  CODE_SIGN_STYLE=Automatic \
  -allowProvisioningUpdates \
  -allowProvisioningDeviceRegistration \
  build

app_path="$derived_data/Build/Products/Release-iphoneos/Forge.app"
bundle_path="$app_path/main.jsbundle"

if [ "$dry_run" -eq 0 ]; then
  [ -d "$app_path" ] || fail "Release app was not produced at $app_path"
  [ -s "$bundle_path" ] || fail "Release app does not contain an embedded JavaScript bundle"
  bundle_magic=$(od -An -tx1 -N4 "$bundle_path" | tr -d ' \n')
  [ "$bundle_magic" = "c61fbc03" ] ||
    fail "embedded bundle is not Hermes bytecode (unexpected header $bundle_magic)"
  actual_bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app_path/Info.plist")
  [ "$actual_bundle_id" = "$bundle_id" ] || fail "built bundle ID is $actual_bundle_id, expected $bundle_id"
  codesign --verify --deep --strict --verbose=2 "$app_path"
  signature_info=$(codesign -dvv "$app_path" 2>&1)
  case "$signature_info" in
    *"TeamIdentifier=$team_id"*) ;;
    *) fail "signed app does not use development team $team_id" ;;
  esac
  codesign -d --entitlements :- "$app_path" >/dev/null 2>&1 || fail "signed entitlements could not be read"
fi

run xcrun devicectl device install app --device "$device_id" "$app_path"
if [ "$launch" -eq 1 ]; then
  run xcrun devicectl device process launch \
    --device "$device_id" \
    --terminate-existing \
    --activate \
    "$bundle_id"
fi

if [ "$dry_run" -eq 1 ]; then
  printf 'Dry run complete. No project, build, app, or iPhone state was changed.\n'
  exit 0
fi

printf 'Forge was built as a signed Release app with an embedded bundle.\n'
if [ "$launch" -eq 1 ]; then
  printf 'Forge was installed and launched on the selected iPhone.\n'
else
  printf 'Forge was installed on the selected iPhone.\n'
fi
