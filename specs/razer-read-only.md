# Experimental Razer reads

This adapter reads firmware version and current X/Y DPI. It has no setters and is
not a claim of working DPI/RGB configuration for 102 mice. No physical device was
available for this implementation; tests use independently constructed synthetic
wire fixtures, not USB captures. The 99% combined configuration goal remains open.

Routing facts are extracted from OpenRazer commit
`477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d`:
[mouse driver](https://github.com/openrazer/openrazer/blob/477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d/driver/razermouse_driver.c),
[common reports](https://github.com/openrazer/openrazer/blob/477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d/driver/razerchromacommon.c).
The Rust implementation describes wire facts independently; upstream driver code
is not linked or executed. Upstream license and catalog attribution remain in
research/licenses and research/NOTICE.md.

`scripts/Build-Razer-Read-Profiles.py` requires this exact checkout revision. It
extracts explicit per-model transaction IDs, interface exceptions and response
waits. It joins DPI maximum declarations from the provenance-bearing catalog.
Only current-XY getters using NOSTORE are selected; legacy formats and Bluetooth
are excluded. The 102 entries are USB PID/routing profiles, not unique verified
retail models. Declared maxima are source checks, not runtime-discovered ranges.

| Payload field | Rule |
|---|---|
| Size | 90 bytes, unnumbered feature report; hidapi buffer includes leading ID 0 |
| Transaction | Exact model-specific value from the compiled profile |
| Remaining / protocol | Zero |
| Firmware request | Class 00, command 81, size 2, zero arguments |
| DPI request | Class 04, command 85, size 7, NOSTORE selector 0 |
| Checksum | XOR payload bytes 2 through 87 at byte 88; reserved byte 89 zero |
| Response | Exact size, transaction/header correlation, checksum, success 02 |

Before requests: USB bus, vendor 1532, reviewed PID, exact USB interface and
descriptor feature ID 0 / 90-byte payload are required. The native transport
allows exactly the firmware request followed by the DPI request once. A changed
byte, wrong order, third request, short response, busy/error or disconnect stops
the probe. No retry, receiver-slot guessing or device-mode switch is attempted.
Strict correlation may reject otherwise usable firmware; such rejection grants
no fallback permission. A protocol response does not prove physical DPI behavior.

These are read operations at the protocol level, but they send two documented
GET-command requests over HID SET_REPORT before reading responses. They are not
passive USB observation. Normal scan/startup never performs this probe. The GUI
offers a separate experimental read button on matching interfaces; CLI requires
`razer-probe --device N --experimental`. `razer-probe --demo` uses memory only.

GUI read success saves an observation to diagnostics with exclusive creation;
it contains no serial number or USB path. It never installs writable capabilities,
creates a settings backup or enables effects. Starting the probe stops the old
effect/scene and releases the active control session. A read report is not a backup.

The inspected upstream XY setter takes a storage parameter but hardcodes VARSTORE.
Persistence and failure behavior remain unmeasured. Neither that setter nor RGB,
profile, polling, firmware, reset or bootloader commands are exposed here.
