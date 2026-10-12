#!/bin/bash
# shot.sh <out.png> : screenshot of THIS project's OptChat window only (matched by process path, never another app)
pid=$(pgrep -f "optchat-client/(dist-dev/OptChat Dev.app/Contents/MacOS|target/(debug|release))/OptChat" | head -1)
[ -z "$pid" ] && { echo "no OptChat (gpui) process"; exit 1; }
W="$(dirname "$0")/winid"; [ -x "$W" ] || swiftc -O "$W.swift" -o "$W" 2>/dev/null # built on first use (git-ignored)
id=$("$W" "$pid") || { echo "no window"; exit 1; }
screencapture -x -o -l "$id" "$1"
