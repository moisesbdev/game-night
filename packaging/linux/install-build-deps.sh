#!/usr/bin/env bash
# The libraries the Linux window needs to build (WebKitGTK and GTK). Used by CI and the release workflow.
# Each try gets five minutes, up to three tries: an Ubuntu mirror that hangs can't stall a whole build.
set -u
for try in 1 2 3; do
  if timeout 300 sudo apt-get update -o Acquire::Retries=3 &&
     timeout 300 sudo apt-get install -y -o Acquire::Retries=3 libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev; then
    exit 0
  fi
  echo "Installing the window libraries failed or hung (try $try of 3); trying again." >&2
  sleep 15
done
exit 1
