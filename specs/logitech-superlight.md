# Logitech G PRO X SUPERLIGHT 1

Status: **unverified**. Public description, not self-captured. Retrieved 2026-10-23.

Identity source: https://github.com/libratbag/libratbag/blob/master/data/devices/logitech-g-pro-x-wireless-superlight.device
lists 046D:C094 and logical wireless identity 046D:4093 with HID++2. Do not assume 4093
is the physical receiver PID. Initial experimental support is wired C094 only.
Receiver routing, pairing and slot discovery are deferred.

Framing source: https://github.com/Logitech/cpg-docs/blob/master/hidpp20/README.rst

| Offset | Length | Meaning |
|---|---|---|
| 0 | 1 | Report ID: 10 short (7 total), 11 long (20 total) |
| 1 | 1 | Target; FF for wired device |
| 2 | 1 | Runtime feature index |
| 3 | 1 | Function high nibble, nonzero software ID low nibble |
| 4 | 3 or 16 | Parameters, zero padded |

Feature lookup source: https://github.com/Logitech/cpg-docs/blob/master/hidpp20/features/0x0000-IRoot.rst
Root index 0, function 0, feature ID big-endian; response parameters: index, flags, version.
Zero index means unavailable. Ping uses root function 1 with an echoed third parameter.

DPI source: https://lekensteyn.nl/files/logitech/x2201_adjustabledpi.html
Feature 2201, sensor 0. Functions: 0 count; 1 supported list; 2 current DPI;
3 set DPI. Set parameters: sensor, DPI MSB, DPI LSB. Read response repeats sensor then
current DPI MSB/LSB. Version 0 may omit set echo; always read back after a write.
List uses big-endian words; E001..FFFF encode steps between preceding/following endpoints.
Reject malformed ranges and values outside the advertised set.

Errors reference: https://github.com/abd3lraouf-studios/logitech-layout-auto-switcher/blob/main/docs/PROTOCOL.md
Error envelope: report, device, FF (HID++2) or 8F (HID++1), original feature, original
function/software byte, status. Match the original request before interpreting status.

Descriptor bytes, Windows collection, maximum report padding and actual 2201 availability:
**unverified**. Experiment: perform the common capture plan on wired C094; confirm vendor
usage page and short/long support. Application must refuse absent features, unsupported DPI
and readback mismatch. No fallback to onboard profiles or persistent memory writes.

RGB, polling and remap are not implemented. Power LED is not treated as RGB.
