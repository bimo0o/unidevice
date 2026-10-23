# Corsair Harpoon RGB PRO: bounded live investigation

2026-10-23. User connected the mouse and authorized testing. Enumeration confirms
USB 1B1C:1B75, interface 1, usage FFC2:0004. No settings changed at enumeration.

First experiment is read-only: require an unnumbered 64-byte feature report in
the selected collection's descriptor, then send exactly these zero-padded queries:
`0e 01 00 00` (identity), `0e 13 02 00` (live stage), `0e 13 05 00` (mask),
`0e 13 d0..d5 00` (stage parameters). No scan of undocumented subcommands.
hidapi includes report ID 0 as a separate first byte: buffer length 65.
Receive exactly 65 bytes with ID 0; require the four echoed command bytes.
Serial requests, 20 ms send/receive gap, no retries after any failure.
Record only firmware, VID/PID, stage/DPI/RGB responses and descriptor report sizes;
never persist USB paths, serials or the uninterpreted identity buffer.

Sources (public descriptions, not implementation code):
- https://github.com/OpenMouse-Project/mouse-protocol/blob/main/captures/corsair-nightsword/PROTOCOL.md
  documents feature transport on FFC2:0004 and live reads; verified on a DIFFERENT
  model (Nightsword). Treat Harpoon applicability as unverified until measured.
- https://github.com/ckb-next/ckb-next/wiki/Maintainers-Corsair-Protocol
  documents the same queries, including an older LE stage-DPI interpretation.
- https://holland.sh/post/crafting-drivers-libusb/ describes original Harpoon
  1B3C (NOT PRO), live stage writes and RGB; no driver source copied.

Potential later setter: `07 13 dN 00 XY Xlo Xhi Ylo Yhi R G B`, zeros through
byte 63. Preserve all untouched fields read immediately before the write. Permit
only the currently active enabled stage, no stage/mask changes, no flash commands.
Nightsword captures confirm flag 0 is volatile; Harpoon persistence is still
unverified. Do NOT enable this setter until read-only Harpoon results establish
unambiguous stage state. Confirm each setter using fresh matching GET responses.
Do not infer logo zone IDs from the original Harpoon. Dedicated logo RGB remains
blocked until its model-specific mapping/transport is established.

This independently worded note attributes the protocol documentation above and
is distributed under CC BY-SA 4.0, matching the existing Corsair research note.
