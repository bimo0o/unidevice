# Settings checkpoint and recovery

User request: detect failed configuration, retain a backup, restore where safe.
This is settings recovery, not a firmware image or a guaranteed anti-bricking system.
Readback demonstrates protocol state and response, not correct motion/click behavior.

Before the first modifying operation in a live selected session, read fresh supported
settings and create a new PortableProfile v2 in the executable's backups/ directory.
File creation must be exclusive and sync must succeed before the first setter.
If reading or saving fails, suspend the session and do not send a setter. One baseline
is retained for that session, not one file per animation frame. Subsequent setters
still perform the adapter's existing fresh-state and full readback checks.

Backup fields: supported DPI and RGB only. No raw packets, firmware, onboard profiles,
serials, device paths, input history or automatic activation. Independent Corsair X/Y
or XY mode cannot be represented by a portable scalar DPI: omit that DPI from the
file instead of silently flattening the axes. The private in-memory snapshot retains
the exact active-stage state; restoration remains inside the adapter's DPI policy.
Demo keeps an in-memory snapshot and writes no backup file.

The Restore button is an explicit action. It stops animation and uses the same
still-active device session. Corsair restores only the same active DPI stage, using
one existing live-stage setter with a fresh precondition and complete readback.
HID++ uses the existing advertised-DPI transaction. Firmware/flash/reset/mode/save
commands are not introduced. Restore failure latches control; it never retries.

Faulted or disconnected sessions cannot restore. Explicit refresh/reprobe discards
the in-memory checkpoint so it cannot silently target a different device. Persistent
files remain loadable as profile drafts after restart/reprobe: review supported fields
and explicitly apply them. This is a manual settings transfer, not proof of original
physical-device identity. Never automatically restore an imported file or on reconnect.

Scenes create each member's baseline before starting scene writers. A stopped scene
joins and closes its member sessions; use saved profile drafts after explicitly selecting
and checking a member again. There is no automatic group rollback or device reattachment.

A device that no longer enumerates on USB cannot receive recovery settings from this
application. Such failure may require a vendor-specific recovery procedure; UniDevice
does not experiment with bootloader/DFU commands.

Acceptance: unchanged baseline across multiple edits; explicit restore; new selection
invalidates memory binding; stage mismatch emits no setter; mismatched restore readback
faults without retries; progress metadata never unlocks a pending GUI command. Physical
validation is pending because the user currently has no devices available.
