# Device safety policy for interactive control

User priority, 2026-10-23: minimize device-bricking risk. An absolute zero-risk guarantee
is impossible without device/firmware validation; firmware may itself have defects.
VirusTotal results reported clean by the user concern malware, not HID command safety.

The transport must reject every unknown outgoing command, not just hide it in the GUI.
Allowed family: HID++2 root ping, lookup of Adjustable DPI 2201, sensor count, advertised
DPI list, active DPI read and one explicitly requested sensor-0 DPI change. Runtime feature
indexes must be learned from correlated replies; reserved indexes rejected. Payload format
remains as specified in logitech-superlight.md. No arbitrary report/feature/IOCTL endpoint.

Restrictions: wired target FF only; descriptor verified; no receiver slot guesses; no
onboard profiles, flash, firmware, reset, pairing, bootloader, hardware-profile save or
vendor-mode changes. RGB protocols without complete transport/init/zone specifications
remain read-only. Do not assume a generic HID output is RGB.

Harpoon exception, physically measured 2026-10-23: USB 1B1C:1B75 firmware 3.13,
FFC2:0004, unnumbered feature64. Allow identify, live stage/mask/stage reads and
one live active-stage rewrite (07 13 dN 00), retaining untouched fields. DPI policy
100..12000 step100; RGB edits preserve X/Y and XY flag. No stage/mask changes,
hardware profile flag, mode switches or inferred logo zone IDs. Readback required.
800/1000 DPI round-trip passed; blue stage RGB readback and user visual check passed.
Original 1000 DPI/red restored explicitly in the lab; GUI never auto-restores.
Persistence across reconnect is not verified. See corsair-live-probe.md.

Every apply validates fresh capabilities and reads current state before a single setter;
the value must belong to the device's list and conservative application policy. Never write
on slider movement, discovery, reconnect, profile load, startup or demo. No retry loop,
automatic rollback or hardware save. Periodic RGB is allowed only under the explicit
animation policy in rgb-effects.md; periodic DPI setters remain forbidden. After ambiguous write/read failure,
close the handle and require an explicit refresh/probe before another apply.

GUI uses opaque in-memory device paths to bind a selection; never persistent numeric indexes.
Revalidate VID/PID/usage and descriptor on the selected handle before use. Disconnect/replug
must not silently retarget a different device. Unique paths and serials are never printed
or persisted. Each device channel serializes its own operations; only one manual
apply can be pending. Scene channels share a fail-stop flag and are joined before
manual control resumes. Local compatibility packs cannot extend the compiled
command allowlists, VID families or firmware gates. Import/preview never applies.

Tests must demonstrate that unknown, reset/bootloader-like, long, wrong-route, out-of-range,
unadvertised and out-of-sequence packets never reach a fake device. Invalid readback must
not be reported as success. Hardware verification remains unverified until captures exist.
