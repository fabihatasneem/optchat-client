#!/bin/bash
# Archive OptChat and upload it to App Store Connect (TestFlight), signed and uploaded with the team's API key.
#   release/testflight.sh [build-number]      default build number: UTC yyMMddHHmm (monotonic)
# Version: MARKETING_VERSION in OptChat.xcodeproj (0.1.0). Logs: release/logs/ (git-ignored). Never prints the key.
# The App Store Connect API key config is OUTSIDE the repo: OPTCHAT_ASC_JSON (default ~/.config/optchat/asc.json),
# {keyId, issuerId, keyPath, teamId}; see asc.example.json.
set -euo pipefail
cd "$(dirname "$0")/.."
ASC_JSON="${OPTCHAT_ASC_JSON:-$HOME/.config/optchat/asc.json}"
[ -f "$ASC_JSON" ] || { echo "no App Store Connect key config at $ASC_JSON (set OPTCHAT_ASC_JSON)" >&2; exit 1; }
BUILD="${1:-$(date -u +%y%m%d%H%M)}"
read -r KEYID ISSUER KEYPATH TEAM < <(python3 -c "import json,sys;d=json.load(open(sys.argv[1]));print(d['keyId'],d['issuerId'],d['keyPath'],d['teamId'])" "$ASC_JSON")
AUTH=(-allowProvisioningUpdates -authenticationKeyPath "$KEYPATH" -authenticationKeyID "$KEYID" -authenticationKeyIssuerID "$ISSUER")
mkdir -p release/logs build
ARCHIVE="build/OptChat-$BUILD.xcarchive"
echo "$(date +%T) archive build $BUILD"
xcodebuild archive -project OptChat.xcodeproj -scheme OptChat -configuration Release -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE" "${AUTH[@]}" DEVELOPMENT_TEAM="$TEAM" CURRENT_PROJECT_VERSION="$BUILD" > release/logs/archive-$BUILD.log 2>&1 \
  || { echo "archive failed: release/logs/archive-$BUILD.log"; grep -E "error:" release/logs/archive-$BUILD.log | head -5; exit 1; }
cat > build/ExportOptions.plist <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>upload</string>
  <key>teamID</key><string>$TEAM</string>
  <key>signingStyle</key><string>automatic</string>
  <key>uploadSymbols</key><true/>
  <key>manageAppVersionAndBuildNumber</key><false/>
  <key>testFlightInternalTestingOnly</key><false/>
</dict></plist>
PL
echo "$(date +%T) export + upload"
xcodebuild -exportArchive -archivePath "$ARCHIVE" -exportOptionsPlist build/ExportOptions.plist -exportPath build/export-$BUILD "${AUTH[@]}" \
  > release/logs/upload-$BUILD.log 2>&1 || { echo "upload failed: release/logs/upload-$BUILD.log"; grep -E "error|Error" release/logs/upload-$BUILD.log | head -8; exit 1; }
grep -E "Upload succeeded|EXPORT SUCCEEDED|Uploaded" release/logs/upload-$BUILD.log | head -3
echo "uploaded OptChat 0.1.0 ($BUILD); App Store Connect processes it (release/asc.py builds com.reframe.optchat)"
