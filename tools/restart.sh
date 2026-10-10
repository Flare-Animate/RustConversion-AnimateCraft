#!/bin/sh
# Restart the app under Xvfb :99 (used by UI smoke tests).
pkill -x animatecraft; sleep 0.5
cd "$(dirname "$0")/.." && (DISPLAY=:99 nohup target/release/animatecraft > out/app.log 2>&1 &)
sleep 3
