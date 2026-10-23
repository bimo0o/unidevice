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
