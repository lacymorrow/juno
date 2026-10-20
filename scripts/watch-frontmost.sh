#!/usr/bin/env bash
# Did anything take the person's screen?
#
# Background mode's whole claim is that the app you are typing in stays the app
# you are typing in. This watches that claim directly: it samples the frontmost
# app and its key window several times a second and prints a line every time
# either one changes.
#
# The acceptance test for LAC-5007:
#
#   1. Open Notes, type something, leave it frontmost.
#   2. scripts/watch-frontmost.sh &
#   3. In Juno, background mode on: "attach receipt.pdf and quote.pdf from
#      ~/Desktop/<folder> to a new email, leave To empty, don't send."
#   4. Keep typing in Notes the whole time.
#
# Pass: the compose window appears with both files, every line below still says
# Notes, and your typing all landed in Notes. One line naming another app is
# the bug (focus theft), whoever took it.
#
# Usage: watch-frontmost.sh [seconds]   (default 120)

set -uo pipefail

seconds="${1:-120}"
interval=0.25

read_front() {
	osascript <<-'APPLESCRIPT' 2>/dev/null
		tell application "System Events"
			set p to first application process whose frontmost is true
			set appName to name of p
			set winName to "(no window)"
			try
				set winName to name of front window of p
			end try
		end tell
		return appName & " — " & winName
	APPLESCRIPT
}

started=$(date +%s)
deadline=$((started + seconds))
last=""
changes=0

printf 'Watching the frontmost app for %ss. Ctrl-C to stop.\n\n' "$seconds"

while [ "$(date +%s)" -lt "$deadline" ]; do
	now=$(read_front)
	if [ -n "$now" ] && [ "$now" != "$last" ]; then
		elapsed=$(($(date +%s) - started))
		if [ -n "$last" ]; then
			changes=$((changes + 1))
			printf '%4ss  CHANGED  %s\n' "$elapsed" "$now"
		else
			printf '%4ss  start    %s\n' "$elapsed" "$now"
		fi
		last="$now"
	fi
	sleep "$interval"
done

printf '\n'
if [ "$changes" -eq 0 ]; then
	printf 'PASS: the front never changed. %s kept the keyboard the whole time.\n' "${last%% — *}"
	exit 0
fi
printf 'FAIL: the front changed %s time(s). Background mode let something take the screen.\n' "$changes"
exit 1
