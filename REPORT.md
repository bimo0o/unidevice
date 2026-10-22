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

## Change 4 — adaptive specification (phase 1 amendment)

- File: `specs/adaptive-discovery.md`: +39 lines.
- Check: `git diff --cached --stat`: one specification, no code.
- Requirement: user clarified that new models must adapt without a model table.
- Risk: vendor-specific semantics cannot be inferred from an HID descriptor alone.
- Commit: `251b323 docs: specify model-independent capability discovery`.
- Next: descriptor-based adapter selection and dynamic capabilities.

## Change 5 — adaptive test product (phase 4)

- Files: `src/{descriptor,controller,profile,device,main,hidpp,lib}.rs`, `profiles/`, `tests/cli.rs`.
- Check: `cargo test --offline`: 25 tests pass; `cargo clippy --offline --all-targets -- -D warnings`: pass.
- Requirement: default auto adapter uses no PID whitelist, parses descriptor layouts,
  queries runtime DPI capabilities; JSON profiles, CLI menu, offline simulation.
- Risk: hardware unverified; arbitrary new protocols, Corsair writes and Gembird control
  are explicitly unsupported. Third fully operational model cannot be claimed.
- Commit: the commit containing this section (`git log --oneline -- src/descriptor.rs`).
- Next: portable builds, resource measurements, packaging and final support matrix.

## Change 6 — build and distribution tooling (phase 5)

- Files: `scripts/`, `.github/workflows/ci.yml`, `.cargo/config.toml`, `packaging/`,
  `licenses/`, `THIRD_PARTY.md`, README updates.
- Check: local Windows GNU release and static Linux MUSL cross-compilation succeeded.
  Windows release EXE subsequently disappeared after a sharing violation; cause not established.
  Debug Windows binary remains runnable. Antivirus settings were not changed.
- Requirement: portable binaries, reproducible builds, dependency notices, CI on both OSes,
  external idle measurement script; no application network code.
- Risk: Linux runtime not available locally; CI will perform native validation. MSVC CI
  uses static CRT configuration, distinct from the local GNU toolchain.
- Commit: the commit containing this section (`git log --oneline -- scripts/Build.ps1`).
- Next: run CI, validate downloadable binaries, measure and package the tested artifact.

## Change 7 — measurements, final status and distribution hold (phase 5/6)

- Files: `SECURITY_REVIEW.md`, `reports/`, packaging hold, workflow and README updates.
- Check: before receiving the antivirus details, `scripts/Measure.ps1` on Windows debug:
  10.104 s sampling after 2 s warmup, 8 logical CPUs, menu blocked on stdin, no HID opened.
  Max working set 5,238,784 bytes (~5.00 MiB), private bytes 847,872 (~0.81 MiB).
  Measured CPU delta 0 ms (0% at measurement resolution); binary 10,728,922 bytes (~10.23 MiB).
- Windows release compilation succeeded; observed file size before deletion: 412,672 bytes.
  This is NOT a usable/approved release artifact. User confirmed Kaspersky heuristic deletion.
- Linux x64 MUSL cross-build: 546,944 bytes (~0.52 MiB); local Linux execution/RAM/CPU:
  NOT RUN. Linux runtime cannot be inferred from cross-compilation.
- Integrity: 24 crate archives match Cargo.lock checksums. OSV: 0 published matches in
  24 version queries; not a malware scan. Details and limitations in SECURITY_REVIEW.md.
- Requirement: transparent limits, actual measurements, no antivirus bypass or unsafe release.
- Risk: antivirus verdict unresolved. Packaging suspended. No physical-device verification.
- Commit: the commit containing this section (`git log --oneline -- SECURITY_REVIEW.md`).
- Next: vendor reanalysis and independent build-chain review; then hardware validation.

## Final capability matrix

| Adapter/model | Function | Software check | Hardware status |
|---|---|---|---|
| Previously unlisted Logitech HID++ model | Descriptor/runtime DPI discovery | mock-tested without PID whitelist | unverified; wired target FF only |
| Superlight 1 | DPI set/read and supported list | mock-tested; opt-in guarded | unverified; no onboard/receiver fallback |
| Harpoon RGB PRO | RGB payload preview | fixture-tested | writes disabled; report ID/init/zones missing |
| Harpoon RGB PRO | DPI | not implemented | requires protocol evidence |
| Unknown Gembird | Descriptor inspection | parser-tested | actual device unavailable; control disabled |
| Arbitrary new proprietary protocol | Automatic configuration | unsupported | descriptor alone cannot reveal semantics |

## Clean-room checklist

- [x] Model specs and adaptive spec committed before their protocol implementations.
- [x] Public references attributed; all hardware claims marked unverified.
- [x] No manufacturer SDK, copied community driver implementation or proprietary binary in Git.
- [x] No serial numbers/device paths in application output or saved profiles.
- [x] No application network, telemetry, account or service.
- [x] Development downloads and package vulnerability queries recorded separately.
- [ ] Physical captures, report/zone verification and measured device behavior.
- [ ] Independent clean-room team separation (one agent performed both stages).
- [ ] Security verdict resolved and portable release validated/distributed.

Excluded: unknown vendor protocol synthesis, receiver slot discovery, firmware/reset/flash,
macros/remap/polling, decorative RGB on Superlight, Gembird control before identification,
GUI and startup daemon. Scope avoids inventing undocumented commands and background load.
The code/test product exists; a ready distributable Windows product is **not complete**.

## Change 8 — hosted verification and removal of generated CI binaries

- Files: this report and `SECURITY_REVIEW.md`.
- Check: [CI run 39873451484](https://github.com/bimo0o/unidevice/actions/runs/39873451484),
  commit `ad4f127`: Windows and Ubuntu jobs both succeeded (tests, fmt, Clippy,
  release compilation, offline demo). This verifies native Linux execution in CI,
  not local Linux RAM/CPU or physical-device behavior.
- Check: removed the two generated binary artifacts from that run; API artifact count = 0.
- Requirement: preserve reproducible source verification while holding distribution.
- Risk: antivirus detection unresolved; successful CI is not an antivirus verdict.
- Commit: the commit containing this section (`git log --oneline -- REPORT.md`).
- Next: vendor reanalysis and independent review of the exact intended release artifact.

Private source repository: https://github.com/bimo0o/unidevice . No release published.
