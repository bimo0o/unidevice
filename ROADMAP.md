# UniDevice Studio status

Implemented in 0.3: model-independent HID++2 discovery within its known protocol
family; local compatibility metadata packs with revision backups and immutable
hardware gates; portable DPI/RGB/effect/feel profiles with explicit compatibility
review; cyclic keyframe editor, import/export and local animated preview; shared
RGB scenes with per-device brightness/phase, independent serial device channels
and one common epoch/fail-stop flag; capability passports and sensitivity wizard.

Per-application automation is intentionally excluded by the user.

Physical evidence: Harpoon RGB PRO firmware 3.13 DPI and RGB, including measured
31.52fps from the earlier pacing test. Studio multi-device scheduling is tested
with two independent simulated devices. Real multi-brand synchronization still
requires more physical RGB-capable devices. No claim of universal unknown-protocol
decoding, signed online compatibility updates or certified raw-input DPI calibration.

Future extension requires measured protocol specs before implementation. Unknown
protocols, receiver routes and firmware versions remain blocked. No automatic
reconnect/startup apply or effect resume. OS input counters are never input history.
