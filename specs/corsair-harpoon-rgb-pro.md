# Corsair Harpoon RGB PRO (wired)

Current status: live DPI and active-stage RGB verified on the user's firmware 3.13.
Authoritative measured specification: [corsair-live-probe.md](corsair-live-probe.md).
The remaining text is the historical recon note; its general zone RGB preview is
not the active-stage RGB implementation. Other firmware and persistence unverified.
Retrieved 2026-10-23.
Identity: 1B1C:1B75; https://github.com/ckb-next/ckb-next/wiki/Supported-Mice
Do not confuse with original Harpoon or wireless variants.

Protocol reference: https://github.com/ckb-next/ckb-next/wiki/Maintainers-Corsair-Protocol
NXP v2 payload is 64 bytes with zero padding. RGB candidate:

| Offset | Length | Meaning |
|---|---|---|
| 0 | 1 | 07 write |
| 1 | 1 | 22 submit mouse color |
| 2 | 1 | Number of zone entries |
| 3 | 1 | 01 |
| 4+4n | 1 | Zone ID |
| 5+4n | 3 | Red, green, blue |

Software-lighting candidate: `07 05 02 00 01`, padded to 64 bytes.
Firmware-info query: `0e 01`, response echoes four header bytes; identity at offsets 12..15,
little-endian. Write commands do not promise an ACK.

**Unknown:** report ID, correct HID collection, zone mapping, initialization sequence,
RGB readback, DPI write semantics and applicability to this SKU. A 64-byte payload must
not be confused with a leading HID report ID. Therefore RGB is an offline packet preview
only; no Corsair hardware write is implemented. DPI/remap/polling: unsupported.

Experiment: capture three primary colors in iCUE, determine descriptor and every setup
packet, enumerate zones, compare USB transport framing, verify reconnect behavior.
Do not infer a zone ID or send hardware-profile/flash commands.

Additional documentation: https://github.com/ckb-next/corsair-protocol
(CC BY-SA 4.0; no text/code copied). This independently worded research note attributes
the observations above and is shared under CC BY-SA 4.0:
https://creativecommons.org/licenses/by-sa/4.0/
