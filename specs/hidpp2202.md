# HID++ extended DPI 0x2202

Status: experimental implementation with synthetic fixtures, no physical tests.
This adds DPI, not RGB or measured market coverage.

Protocol facts were checked against OpenLogi commit
`b2a4dbba3594f8dbcd9fe20cb5c56a40baa33511`:
[implementation](https://github.com/AprilNEA/OpenLogi/blob/b2a4dbba3594f8dbcd9fe20cb5c56a40baa33511/crates/openlogi-hidpp/src/feature/extended_dpi.rs)
and its types module. General framing follows
[Logitech documentation](https://github.com/Logitech/cpg-docs/blob/master/hidpp20/README.rst).
Sources were inspected statically, never installed or executed. Our wire implementation
and safety state machine are independent. Attribution and MIT license are retained
in research/NOTICE.md and research/licenses/OpenLogi-MIT.txt.

## Discovery

Existing USB Logitech VID 046D / vendor collection FF00 gates apply, with no PID
whitelist. Target FF only; receiver routing remains unsupported. Prefer 0x2201;
only an explicit absent-feature reply permits lookup of 0x2202. Timeout, error,
hidden feature or bad response never selects another protocol. Extended version
must be 0; its runtime index comes from the root response.

Native transport checks its own descriptor. Extended lookup additionally requires
input/output report 11 with exactly 152 payload bits (20 bytes including ID), plus
existing short framing. Short-output-only collections cannot gain long writes.

## Permitted transaction

| Function | Rule |
|---|---|
| 0 | Sensor count 1–8; only sensor zero addressed |
| 1 | Fresh capabilities; reject unknown flag bits |
| 2 | X and advertised Y ranges; sequential pages 0–15 |
| 5 | Current/default X/Y DPI and lift-off; long reply required |
| 6 | One explicit DPI update followed by function 5 readback |

Range words may cross 13-byte page boundaries; never pad pages separately. Require
an explicit zero terminator and zero trailing bytes, ordered bounded ranges and
nonzero steps. Shared endpoints are deduplicated. The requested DPI must occur in
both advertised axis sets. Application policy remains 100–25600.

The UI currently represents one DPI number. Unequal current X/Y settings are
refused rather than flattened or backed up incorrectly. Equal independent axes
are changed together; unsupported Y stays zero. Lift-off is preserved from fresh
state. Current/default DPI and optional fields must agree with capabilities.

The firewall independently observes complete capabilities, ranges and current
state before reconstructing the exact permitted long setter. Original DPI must
also belong to the restorable set and policy. Reserved bytes stay zero. Only one
setter may be outstanding. Readback must match requested axes, original defaults
and lift-off; ACK alone is insufficient. Success clears write prerequisites.
An error or unsolicited parameter/calibration event stops control without retry
or automatic rollback. New writes require fresh reads.

No functions 3/4/7–15, profile mode, calibration, indicator LED, flash, pairing,
firmware or reset commands are exposed. No NV commit is issued. Persistence after
power cycles is unmeasured and is not promised. Protocol success cannot certify
firmware safety; physical captures and counts/distance tests remain necessary.
Studio and CLI use the existing baseline-before-modification path; restoration
remains explicit and bound to the active session.

Tests cover both optional-axis variants, the full transaction through the native
firewall, split words/shared endpoints, malformed pages, unknown versions/flags,
asymmetric axes, changed defaults/lift-off, every altered setter byte, forbidden
functions, missing prerequisites, timeout and duplicate writes. Fixtures are
constructed data, not USB captures.
