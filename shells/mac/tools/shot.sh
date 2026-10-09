#!/bin/bash
# shot.sh <out.png> : screenshot of the OptChat window only (never the whole screen)
id=$("$(dirname "$0")/winid" "${2:-OptChat}") || { echo "no OptChat window"; exit 1; }
screencapture -x -o -l "$id" "$1"
