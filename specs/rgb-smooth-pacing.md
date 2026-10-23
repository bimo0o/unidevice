# Smooth RGB: measured transaction pacing, no new commands

User reports visible animation steps. Existing pacing waits 20ms per HID send,
then 250ms after a full verified frame (~2 frames/sec). Do not drop readback or
reuse cached DPI merely to increase frame rate.

First read-only experiment: same identified Harpoon 3.13 feature interface;
query identity, current stage, mask and active stage with 2ms send/receive delay.
Require exact length, report ID and command echo for every reply; stop at first
error, no retry. Never send setters in this experiment. Record actual timing.

Only after that passes: use measured 2ms packet pacing, keep both fresh pre-write
snapshots and full post-write readback, retain physical-stage binding and all
firewalls. Wait 16ms after each completed frame (no catch-up). This limits writes
to <=62.5/sec theoretically; target measured 20–30/sec with USB transaction cost.
Run a bounded gradient test before release, stop on any mismatch, verify no frames
after stop and explicitly restore original RGB only after success. Check unchanged
DPI. Do not restart old animation automatically or run two controller processes.

If 2ms replies fail, retain slow pacing and investigate a documented RGB-only
command from a model-specific capture. Do not guess a lighting-zone index.

Read-only result on the user's Harpoon 3.13: 301/301 correctly correlated replies
in 1188ms with requested 2ms gap; no setters. The user confirmed the old effect
was stopped before measurement. Proceed to bounded verified-frame measurement.

120 frames with 2ms packet gap and 16ms frame wait passed all state checks:
16.63 fps, mean 60.11ms, p95 63.95ms, max 68.07ms. Stop verified, original
1200 DPI / RGB [83,129,255] restored. To approach 30fps without dropping any
checks, perform a second READ-ONLY 301-query experiment at 1ms. Only if all
replies correlate, use 1ms packet pacing and 4ms post-frame wait, then repeat
the bounded full-frame test. The previous 62.5/sec theoretical setter ceiling
remains: add a monotonic 16ms floor between completed frame starts if needed.

Final measurement: 301/301 read-only requests at 1ms matched in 868ms. Then
120/120 full frames passed at 31.52fps (mean 31.73ms, p95 42.24ms, max 48.98ms).
No frame for 700ms after stop; original 1200 DPI / RGB [83,129,255] restored.
Worker subtracts completed-frame duration from a 16ms floor, with >=4ms idle gap.
Slow frames are never followed by catch-up bursts. No validation was removed.
