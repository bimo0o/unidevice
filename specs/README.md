# Protocol research ledger

Date: 2026-10-23. No physical hardware or captures available. Every model-specific claim
and transport choice is **unverified on hardware**. A passing mock test verifies software,
not device compatibility. User authorized sequential phases on 2026-10-23.

| Model | Spec | Hardware status |
|---|---|---|
| Harpoon RGB PRO | [corsair-harpoon-rgb-pro.md](corsair-harpoon-rgb-pro.md) | unverified |
| G PRO X SUPERLIGHT 1 | [logitech-superlight.md](logitech-superlight.md) | unverified |
| Unknown Gembird | [gembird-unknown.md](gembird-unknown.md) | unidentified |

## Evidence rules

Record source URL, retrieval date, connection, firmware version (if supplied manually),
report descriptor and sanitized capture hash when available. Never publish serials, unique
IDs, USB device paths or unredacted captures. Do not capture keyboard input during normal use.
Public model identifiers are not unique identifiers.

## Validation experiment

1. Identify the exact SKU, USB interfaces, VID/PID and report lengths without writing.
2. On a separate capture machine, record a baseline using USBPcap/Wireshark (Windows)
   or usbmon/Wireshark (Linux). Obtain informed permission before collecting input traffic.
3. Change exactly one setting using the vendor application's UI: RGB red/green/blue or
   DPI 400/800/1600. Record before/after values; repeat three times.
4. Identify setup packets, transaction correlation, acknowledgements, timing and readback.
5. Check detach, timeout, permission denial and competing vendor software behavior.
6. Only after descriptor/interface verification, test a documented volatile operation.
   Check visible RGB or measured counts per physical distance, not only an ACK.
7. Confirm whether settings survive reconnect; never assume persistence.

No firmware, flash, bootloader, reset, arbitrary report sender or unknown IOCTL is exposed.
No protocol becomes verified automatically after a successful write.

## General references

- HID format: https://www.usb.org/hid
- Descriptor interpretation: https://kernel.org/doc/html/latest/hid/hidintro.html
- Architecture reference only: https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers
- Rust HID transport: https://docs.rs/hidapi/2.6.7/hidapi/

The experimental opt-in only permits documented commands. It is not permission to guess
unknown packet fields. Missing facts remain blockers in the model-specific specs.
