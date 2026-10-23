# Gembird KBW-G660-1: descriptor-only inspection

User identified the label as KBW-G660-1 and said this is their primary keyboard.
No HID vendor requests, lighting writes, mode switches, reset or firmware operations
were performed. Normal keyboard input was not opened for logging or intercepted.

A connected USB keyboard candidate reports VID 05AC, PID 024F. Its ordinary
keyboard interface exposes input 8/output 1 bytes, unnumbered. Interface 2 also
has a vendor FFFF:0001 collection, input report 5 with 4 bytes. Interfaces 3 and 4
use FF60:0061 with unnumbered input/output of 32 and 64 bytes respectively.
Windows reports OK for the USB composite/input devices. This establishes enumeration
and descriptor accessibility, not a physical key/latency or lighting test.

Shared/OEM USB identity and raw-HID usages do not prove a protocol or a firmware
revision. In particular, FF60:0061 alone never grants VIA/Vial/QMK command permission.
The 1-byte standard keyboard output is not assumed to control decorative RGB.
No command trial, generic RGB write or brand-based firmware rule is enabled.

Official model page:
https://gembird.ru/catalog/keyboards/gaming-keyboards/kbw-g660-1/
It lists 80 keys, USB/2.4GHz/Bluetooth, RGB with 19 modes, brightness/speed controls,
and a model-specific software download. The connection route beyond the observed
USB interfaces and actual firmware revision are not yet established.

The manufacturer links its software at
https://gembird.ru/assets/content/files/resources/552/driver/gembird-kbw-g660-software.rar
and a shared manual at
https://gembird.ru/assets/content/files/resources/552/driver/manual-kbw-g660.pdf
These references do not imply that UniDevice implements their commands. No firmware
download or updater was used. Next: static software/protocol inspection, exact
collection/routing validation, documented non-mutating queries, then capture-backed
lighting semantics and persistence classification before considering a setter.

Research record: reports/gembird-kbw-g660-1-read-only.json. Control remains blocked.
