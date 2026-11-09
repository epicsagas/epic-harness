# Orbit admission and recovery

Persisted pipeline and control JSON accepts ordinary UTF-8 or a single leading
UTF-8 byte-order mark, including files written with Windows PowerShell 5
`Set-Content -Encoding utf8`. This compatibility applies consistently to Orbit
admission, attribution, completion checks and pipeline projections. It does not
relax hook payload validation or accept malformed UTF-8/JSON. Reads preserve
existing files; native writes continue to produce UTF-8 without a marker.

`epic orbit lock` checks for a fresh running pipeline on every acquisition,
including when `.orbit.lock` is absent. A fresh startup reservation remains
exclusive for two minutes while the first pipeline state is written. Admission
and release use a cross-process file lock on Windows and Unix, so concurrent
starts cannot reclaim one another's reservation.

`epic orbit unlock` succeeds only after no fresh running pipeline remains. Mark
the pipeline `complete` or `aborted` before releasing it. An attempted unlock of
an active pipeline returns nonzero and leaves its guard intact. Stale pipeline
recovery retains the documented 45-minute inactivity policy; expired startup
reservations with no active pipeline can also be reclaimed.

For automation that needs ownership checking, set `EPIC_ORBIT_OWNER` to a unique
run token before acquisition and retain the same value for release. A reservation
created with an explicit owner cannot be released with a different or absent
owner. Callers that do not set this optional variable retain the existing
`lock`/`unlock` interface and terminal-state release behavior.
