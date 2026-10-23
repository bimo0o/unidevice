# UniDevice 0.3 accepted feature contract

User selected features 1,2,3,4,6,7,8 on 2026-10-23; per-application automation (5)
is excluded. Preserve measured Harpoon 3.13 pacing, explicit apply and fail-closed I/O.
No new peripheral commands are required for this increment.

1. Adaptive discovery: existing known protocol families, independent of model PID
for HID++2, with descriptor/runtime validation; unknown protocols remain read-only.
Provide guided descriptor diagnostics and local model-metadata package creation.
Never present metadata matching alone as control verification.
2. Shared scenes: explicit device selection, a common monotonic effect clock,
per-device brightness/phase correction, RGB-capability checks. One-device errors
stop the scene; no reconnect auto-resume. Real cross-device latency is not atomic;
expose measured/application timing limits. Demo includes two independent devices.
Scene device channels must run independently so adding a second mouse does not
halve the first mouse's frame rate. Probe all channels before starting writers;
each channel owns its handle and serializes its own verified packets. Share one
epoch and fail-stop flag. Stop/join all channels before manual control resumes.
3. Portable model-independent profiles: DPI/RGB/effect/feel reference, local JSON,
strict schemas/size limits; loading only edits drafts. Show exact/nearest DPI and
unsupported fields before explicit apply. Do not silently substitute a DPI.
4. Local compatibility packs without rebuilding: declarative identity/name/evidence
and narrowing DPI policy for compiled adapters. No executable code, raw packets,
firmware unlock or safety expansion. Imported evidence is an untrusted claim, not
a physical-verified badge. Network auto-updates and signing infrastructure are not
part of this local importer; no claim of authenticated publisher. Unknown protocols
still require an audited compiled adapter. Ship a sample package and editor/import UI.
6. Sensitivity wizard: preserve eDPI within the same game, measured cm/360 reference
and closest supported DPI suggestion; optional raw-motion distance calibration must
identify its limitations and never log input histories. No game hooks or auto-editing.
7. Effect builder: 2–8 cyclic color keyframes, timing/easing, animated local preview,
bounded local import/export, manual apply, no implicit hardware writes on editing.
8. Capability passport: adapter, runtime capabilities, firmware where known, evidence
level, immutable blocked operations, export without serial/path/input history.

Acceptance: meaningful tests for hostile files/policy narrowing, profile mapping,
timeline boundary continuity, shared clock/cancellation/device isolation, sensitivity
math and GUI draft-only behavior. Two-device simulation is not a physical multi-brand
verification. Report remaining physical limitations honestly and ship the runnable UI.
