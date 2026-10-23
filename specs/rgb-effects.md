# Explicit volatile RGB animation policy

User requested gradients/animation on 2026-10-23. This extends the single-apply
policy ONLY for RGB while the user explicitly starts an effect in the current
session. No auto-start on discovery, load, reconnect or replacement device.

Use the already measured Harpoon 3.13 active-stage RGB setter, preserving fresh
DPI/XY data on each frame and verifying the full state after each setter. No new
HID command, lighting mode or flash operation is introduced. Other protocols remain
blocked. Single-zone effects are temporal, not spatial gradients.

Effect engine: static, two-color cosine blend, breathing, rainbow. Pure deterministic
color computation from monotonic elapsed time and cycle length 4..30 seconds.
One serialized worker owns the HID session and animation. Initial 250ms post-frame
pacing was replaced after physical measurements: 1ms per packet, at least 4ms
after a completed frame, and a 16ms minimum frame-start interval. Never catch up
on late frames. All pre-write snapshots and post-write readbacks retained.
Measured 31.52fps for 120 verified frames on Harpoon 3.13. See rgb-smooth-pacing.md.

Stop, DPI apply, static RGB apply, scan, selection, demo switch and shutdown cancel
the effect before further work. Any I/O/readback/state error latches control and
ends animation with no retry or rollback. If the physical DPI stage changes, stop
rather than animate a different stage. Stop leaves the last color; no stale restore.
UI preview reflects successful frames only. No per-frame profile saves or logs.
Physical animation test must be bounded, then original color explicitly restored
only after success. Disconnection never restarts animation automatically.

Tests: deterministic endpoints/periodicity; no frames before explicit start;
stop and subsequent DPI operation cease animation; errors end the loop. Existing
packet/firewall/readback tests remain required.
