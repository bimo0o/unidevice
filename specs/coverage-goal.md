# Mouse protocol coverage goal

Accepted 2026-10-23: target 99% of gaming mice configurable in their manufacturer's
software; success means DPI plus lighting
where lighting is present. Initial platform scope: Windows/Linux, USB cable and USB
radio receiver. Bluetooth requires separately specified routes and is not silently
treated as equivalent to a USB connection. Button assignments are not required for
this success criterion; per-application automation remains excluded.

The probability is currently unknown: no representative market distribution, agreed
benchmark population or model/firmware test sample exists. Do not compute the target
percentage from source-file counts, VID/PID matches or one physical test.
The denominator excludes office mice with fixed hardware DPI. Unsupported gaming
models inside the agreed population remain failures, not exclusions. Population
eligibility and absence of lighting need evidence independent of our own support.

Track separately: source identity recognition, protocol implemented, capability reported
at runtime, DPI setter validated, RGB setter validated or lighting authoritatively absent,
physical model/firmware/transport evidence, recovery side effects and source provenance.
Unknown RGB presence is unresolved, not an automatic pass. Partial DPI-only support
on an RGB model does not satisfy the combined criterion.

Current recognition index: mouse records from libratbag, OpenRazer and ckb-next plus
unambiguous Mouse-category OpenRGB records. VID/PID hints do not select transports or
grant I/O. Some upstream matches are logical receiver identities. Multiple family
matches remain multiple hints. New JSON descriptions never expand native command gates.

Priority inventory is in research/mouse-protocol-priorities.json. It is an engineering
backlog, not market share. Razer, HID++ and Corsair need model/generation/route analysis;
then ASUS, SteelSeries, Sinowealth/OEM and remaining families. Every new setter needs
documented read/write semantics, storage side-effect classification, preconditions,
readback, fault tests and physical validation before a hardware-verified claim.

Source inspection found that OpenRazer commit
477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d driver/razerchromacommon.c
razer_chroma_misc_set_dpi_xy takes a storage parameter but assigns VARSTORE to the
packet. Do not infer volatile behavior from that argument or copy the setter into
our safety policy. Actual persistence/firmware behavior remains unmeasured.
