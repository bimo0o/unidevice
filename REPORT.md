# Engineering report

## Change 1 — scope and specifications (phase 0/1)

- Files: `.gitignore`, `README.md`, `specs/*.md`: +174 lines.
- Check: `git diff --cached --stat`: 6 files, 174 insertions.
- Requirement: specification precedes implementation; all hardware facts unverified.
- Risk: unidentified Gembird; Corsair transport/zone mapping incomplete; wired Logitech only.
- Commit: `86496dc docs: record clean-room scope and unverified device specifications`.
- Next: native transport and discovery.

## Change 2 — native scaffold (phase 2)

- Files: `Cargo.toml`, `Cargo.lock`, `LICENSE`, `src/{lib,main,error,device,transport}.rs`, this report.
- Check: `cargo test`: successful compilation, 0 tests (protocol tests follow in change 3).
- Check: `cargo run --offline -- list`: 0 HID collections in this sandbox; no reports sent.
- Requirement: Rust, Windows/Linux native HID, no service, no serial/path logging.
- Risk: sandbox enumeration is not physical-device validation. RAM/CPU not yet measured.
- Commit: the commit containing this section (`git log --oneline -- REPORT.md`).
- Next: documented HID++ DPI with a mock transport and failure tests.

Toolchain is isolated in ignored `.tools/`. Network is used for development downloads only;
the application has no network code. Git metadata changes require sandbox escalation.

## Change 3 — HID++ DPI protocol (phase 3)

- Files: `src/hidpp.rs`, `src/lib.rs`, `Cargo.toml`, `Cargo.lock`, this report.
- Check: `cargo test --offline`: 11 passed, 0 failed.
- Requirement: runtime feature discovery, capability validation, bounded replies, status
  correlation, malformed packets, short write, timeout/disconnect and readback mismatch.
- Risk: no wired Superlight available; all hardware operations remain unverified.
  Initial DPI policy is conservatively limited to 100..25600, further constrained by device data.
- Transport: use bundled open-source hidapi C backend on Windows. The optional Rust-native
  backend reports padded write lengths; strict short-write validation uses the C backend instead.
- Commit: the commit containing this section (`git log --oneline -- src/hidpp.rs`).
- Next: model routing, safe RGB preview, profiles and usable interactive CLI.
