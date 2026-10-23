# Adaptive capability discovery

Update: 2202 version 0 is also implemented when 2201 is explicitly absent, with
stricter descriptor/axis/state checks. See hidpp2202.md. Both remain physically
unverified in UniDevice; receiver routing and RGB are not granted by this update.

Requirement update from user, 2026-10-23: previously unknown models must be handled
through capability discovery, not a per-model whitelist.

## Safety and evidence

USB HID report descriptors describe report layouts, types, lengths and usages; they do
not necessarily describe the meaning of vendor payloads. Source: https://www.usb.org/hid
and https://kernel.org/doc/html/latest/hid/hidintro.html . Descriptor summaries are read-only.
On Windows hidapi may reconstruct a descriptor from preparsed data; it is not necessarily
the original byte-for-byte descriptor. Source: https://docs.rs/hidapi/2.6.7/hidapi/struct.HidDevice.html

The first adaptive adapter is HID++2 Adjustable DPI. It matches Logitech vendor 046D and
vendor usage page FF00, then checks descriptor framing and queries root feature 2201.
No PID/model table is required. The protocol, functions and validation are specified in
logitech-superlight.md; reuse that public protocol description, not implementation code.

Descriptor gate: output report 10 must total 7 bytes, and input 10 total 7 bytes or input
11 total 20 bytes must exist. Lengths include the report ID. These are conservative
application acceptance rules, not claims that every HID++ product has this layout.
No protocol request is sent before the user opts into --experimental hardware probing.
Only wired target FF is attempted; receivers/slots are not guessed. A failed handshake
or missing/hidden DPI feature leaves control disabled. Capabilities are queried anew on
every operation; no model-derived feature index, DPI range or sensor count is assumed.

Unknown vendors are still enumerated and their descriptors summarized, but no vendor
commands are tried. Usage page 59 (Lighting and Illumination) is an adapter candidate,
not evidence of a complete LampArray implementation. No lighting writes are enabled by it.

## Hardware validation

Status for this whole adapter: **unverified**. Test on at least two previously unlisted
wired HID++ models, compare their descriptors, runtime feature indexes and DPI ranges.
Confirm that unsupported functions, receivers, unrelated HID collections and malformed
descriptors fail before a setter. Physical behavior must be measured separately.

Adaptation to a new model within a documented protocol family is implemented. Autonomous
reverse engineering of arbitrary proprietary protocols is not claimed or attempted.
