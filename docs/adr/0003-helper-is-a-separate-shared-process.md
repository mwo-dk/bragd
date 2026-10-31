# The Helper is a separate process shared by every window

Fetching Sources, indexing and finding Duplicates run in the Helper, a separate background process. It is not a thread inside the app. Every open Bragd window and instance uses the same Helper. It stays ten minutes after the last window closes. The "Start with my session" setting keeps it running through the platform's user service: systemd user unit, launchd, autostart, rc.d or SMF. It survives an upgrade and pauses on battery.

We copied this from coxswain's search helper rather than running the work inside the app. A thread inside the app would stop when the window closes, and two instances would download and deduplicate the same Library twice and fight over its write lock. In return we carry a process lifecycle, a local connection between app and Helper, and the per-platform service files.
