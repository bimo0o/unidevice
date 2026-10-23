# Conditional Logitech RGB8070

Status: experimental implementation and synthetic protocol tests; zero physical
models verified with this adapter. This is not proof of combined market coverage.

Primary implementation facts: OpenLogi commit
`b2a4dbba3594f8dbcd9fe20cb5c56a40baa33511`,
[color_led_effects.rs](https://github.com/AprilNEA/OpenLogi/blob/b2a4dbba3594f8dbcd9fe20cb5c56a40baa33511/crates/openlogi-hidpp/src/feature/color_led_effects.rs)
and its types module. RAM selector 0 was also checked against OpenRGB commit
`817834cfab034c97927451e31c4a2deb60a44725`, LogitechProtocolCommon.cpp.
These sources were read statically, not installed/executed. Our codec/firewall are
independent. Attribution and upstream license texts remain in research/.

## Discovery and requirements

Studio reuses the same USB Logitech FF00 collection, direct target FF and live DPI
session. It looks up 8070 by root query; no model-derived feature/zone/effect index
is used. Native input/output long reports must match the descriptor before lookup.
Version 0–7 common functions only; hidden/absent/newer features disable RGB.
Receivers and RGB8071 are not enabled by this implementation.

Function 0 must report 1–8 RGB zones and extended capability GET_ZONE_EFFECT.
Monochrome-only and unknown extended flags disable RGB. Function 1 reports each
zone's location and 1–32 effects; function 2 enumerates all effect IDs. Each zone
must have exactly one fixed-color effect. All query addressing is correlated.
Geometry/effect metadata is bound to the unchanged live handle; each modification
reads current effects afresh. Mode changes/reconnect never retain that binding.

Function 14 reads RAM source 0 for every zone. Unknown or unrepresentable original
effects disable RGB while DPI may remain available. Current backup/restore layouts:
off (all-zero parameters), fixed RGB (remaining parameters zero), cycle (100–60000ms,
brightness 0–100, zero reserved fields), breathing waveform (same period/brightness
bounds, waveform 0–6, zero reserved fields). Legacy breathing/wave/other effects are
not silently converted. Initial picker color is a first-zone hint, not a measurement
of the visible color of an animated effect.

## Writes and failure behavior

Only function 3 long requests with persistence byte 12 = 0 and bytes 13–15 = 0 are
permitted. This selects RAM according to both inspected sources; firmware persistence
behavior has not been measured. Functions for NV, software ownership, clear/reset,
calibration/bin settings, power/configuration and synchronization are all blocked.

The native firewall must have observed complete zone metadata and a safe current
effect before a setter. It validates actual effect index/type, known parameter
layout, RAM selector and every reserved byte. It permits one outstanding setter
across DPI and RGB. A corresponding full RAM effect readback is required before
another write; an ACK alone is insufficient. Readback consumes that zone's fresh
state. Unknown commands, malformed responses, timeouts or unsolicited RGB events
stop the native channel. Session errors prevent further operations until reprobe.

Static RGB and host animations apply a uniform color to all zones in sequence.
Read all baseline zones before the first setter and check DPI before/after the
operation; no DPI setter is used by RGB rendering. Failure on a later zone can
leave an earlier zone changed: there is no hardware atomicity, retry or automatic
rollback. Scene failures stop/join all channels using the existing scene policy.
No frame-rate or visible-color result is claimed without hardware measurements.

## Recovery and UI

Before the first modification Studio saves DPI and the complete safe per-zone
effect snapshot to a create-new/synced `backups/before-8070-*.json`. Save failure
blocks modification. The snapshot contains settings, no raw command reports,
unique device identifiers or firmware image. Mixed colors/animated effects are
not flattened into a portable RGB backup.

The existing explicit Restore action uses the snapshot only on the still-active
session; unchanged DPI is not rewritten. A fault or reprobe removes that binding.
Disk snapshot import into ordinary portable profiles is not implemented. The file
is retained for inspection/future explicit recovery; it cannot recover a USB brick.
Startup/reconnect never replay it. Native guards are unchanged by JSON files.

Device color, host effects, scene and compatible-field transfers use this adapter
only when discovery and a restorable baseline succeeded. Capability passports use
an explicit adapter type, so RGB Logitech cannot inherit Harpoon firmware 3.13 or
its physical evidence. Only Harpoon retains the existing hardware-verified count.
