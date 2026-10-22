# Gembird keyboard, model unknown

Status: **unidentified / unverified**. Source: user description, 2026-10-23.
VID/PID, revision, report IDs, packet offsets, lighting capabilities and transport:
unknown; **requires descriptor and traffic capture**. No protocol bytes are proposed.

MVP: enumerate HID metadata locally without serial numbers or paths. User identifies the
printed model/SKU and compares VID/PID with the local list. Enumeration alone cannot prove
brand identity: an OEM keyboard may enumerate under another manufacturer's VID.

Experiment: obtain model and descriptors, test physical lighting shortcuts, then (if a
configuration application exists) capture one lighting change at a time. Some keyboards
provide only onboard shortcuts; software RGB must not be assumed. Never send arbitrary
feature reports based on brand name or keyboard input reports.

All writes blocked. No wildcard brand driver. No dependency on vendor SDK.
