#!/usr/bin/env bash
# The libraries the Linux window needs to build (WebKitGTK and GTK). Used by CI and the release workflow.
# Each try gets a time limit, up to three tries: an Ubuntu mirror that hangs or crawls can't stall a whole build.
# GitHub's runners use an Azure mirror; if that one fails, later tries switch to the main Ubuntu archive.
set -u
for try in 1 2 3; do
  if [ "$try" -gt 1 ] && [ -f /etc/apt/apt-mirrors.txt ]; then
    echo "Switching to the main Ubuntu archive." >&2
    echo "http://archive.ubuntu.com/ubuntu/" | sudo tee /etc/apt/apt-mirrors.txt > /dev/null
  fi
  if timeout 300 sudo apt-get update -o Acquire::Retries=3 -o Acquire::http::Timeout=30 &&
     timeout 600 sudo apt-get install -y --no-install-recommends -o Acquire::Retries=3 -o Acquire::http::Timeout=30 \
       libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev; then
    exit 0
  fi
  echo "Installing the window libraries failed or hung (try $try of 3); trying again." >&2
  sleep 15
done
exit 1
