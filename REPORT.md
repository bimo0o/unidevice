# Engineering report

Current work: GUI test build 0.2.0, guarded HID++ DPI, unknown protocols read-only.
Later entries supersede earlier project-status statements. No target model is hardware-verified.

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

## Change 9 — user-requested rebuild for external inspection

- Date: 2026-10-23. User explicitly requested rebuilding for their VirusTotal inspection.
- Source: `f03908730bccc47f7793ccfe4acdb741f8c15b42`; no application code or build flags changed.
- Check: `cargo build --offline --locked --release`: success, 9.19 s.
- Artifact: `target/release/unidevice.exe`, 412,672 bytes; present and readable after build.
- SHA-256: `e4d9b42caeed1850f3f2e0010c946b0e902eb3a47452473e56b13eefad39f88e`.
- MD5: `193fc7645db46ec393d31a7a90561a3c`.
- Execution: NOT RUN. Tests not repeated because application source did not change.
- Requirement: provide an inspection sample at the user's request, not an approved release.
- Risk: original antivirus detection remains unresolved; this sample has a different hash.
  No protection settings changed and no sample was uploaded by the agent.
- Commit: the commit containing this section. Distribution hold remains active.
- Next: user inspects the exact artifact and supplies the external analysis result.

## Change 10 — device safety specification

- File: `specs/device-safety.md`: +31 lines, committed before implementation.
- Check: `git diff --cached --stat` / `git commit`: document-only change.
- Requirement: user's top priority is minimizing device-bricking risk.
- Risk: no absolute zero-risk claim; target firmware/physical devices are not validated.
- Commit: `a257934 docs: define restricted device-control policy before GUI implementation`.
- Next: enforced packet allowlist and a native GUI.

## Change 11 — native GUI and enforced control firewall

- Files: `src/{gui,desktop,safety,transport,hidpp,device,main,error,lib}.rs`, GUI binary,
  Cargo files, build/packaging/measurement scripts, notices/licenses, README and security history.
- Check: `cargo test --offline --features gui`: 36 passed; no failed tests.
- Check: `cargo clippy --offline --features gui --all-targets -- -D warnings`: pass.
- Check: release GUI compiled; native demo screenshot captured through the application's
  own screenshot command and visually inspected. UI drafts do not write until explicit Apply.
- Check: guarded fake transport rejects unknown/wrong-route/long/reset/bootloader-like
  commands, unsupported DPI and retries after timeout. End-to-end fixture passes the firewall.
- Requirement: small native Rust GUI, adaptive DPI, profiles, explicit apply, bounded worker
  operations, immutable selected handle, independent outgoing packet checks, no firmware APIs.
- Resource sample: `reports/gui-resources-windows.json`, 3 s warmup + 10.15 s, demo,
  startup Hidden: max working set 103,768,064 bytes (~98.96 MiB), private 53,526,528
  (~51.05 MiB), 31.25 ms CPU = 0.0385% machine (8 CPUs), 0.3079% one core.
  Sample binary 3,617,792 bytes; final package hashes are generated after final build.
- Read-only native enumeration outside sandbox saw 18 HID collections. No target Logitech
  or Corsair model appeared; no physical setting was changed. Sandbox enumeration alone had
  previously shown zero; this is an access-context difference, not hardware compatibility.
- Risk: real RGB remains disabled; unsupported/unknown devices export metadata only.
  Superlight needs wired USB plus the expected descriptor and feature 2201. No device traffic
  capture, cursor-count measurement, RGB verification or actual DPI write was performed.
- User reports prior VirusTotal sample clean and authorized continued development. That
  result is not independently checked and does not certify newly compiled binaries.
- Commit: the commit containing this section (`git log --oneline -- src/gui.rs`).
- Next: user connects a target mouse, tests discovered DPI, and exports diagnostics if blocked.

## Change 12 — Harpoon RGB PRO live adapter

Pet project, entirely AI-written under human supervision. On 2026-10-23 the user
connected USB 1B1C:1B75 and authorized testing. Spec recorded first (f96853b).
Read-only identification: firmware 3.13, bootloader 3.05; FFC2:0004, feature64.
DPI GET is LE on this firmware. Current-stage trailing bytes are stale and ignored.
Live test: 1000 -> 800 -> 1000 DPI with successful readbacks, RGB/stage retained.
Color: red -> blue for eight seconds -> red; readbacks matched and user confirmed
visible blue/restoration. No firmware/reset/flash/hardware-profile save commands.
Implementation restricts hardware route, firmware, report length, subcommands,
active enabled stage and DPI policy. Single setter then readback, fault latches,
no retries. GUI holds the selected handle and applies only on explicit buttons.
Checks: cargo test --offline --locked --features gui: 38 library + 4 CLI passed.
cargo clippy --offline --locked --features gui --all-targets -- -D warnings: passed.
Risk/limits: physical tests cover this mouse/fw and values 800/1000; replug
persistence, other firmware and remaining DPI values are not verified. Logitech
remains unverified. Universal decoding of arbitrary unknown protocols is not claimed.
Next: portable GUI launch for user testing; effects and cross-device sync later.
Portable Windows GUI built successfully and launched for user testing from
C:/UniDevice/dist/unidevice-windows-x64/unidevice-gui.exe. Process 12460,
MainWindowTitle UniDevice, Responding=True; working set at spot check 110559232 B.
Own-window demo screenshot captured successfully to assets/gui-demo.png.
Archive size 2910533 bytes; GUI SHA256
00cc8650c90e3d4245f0871727d3b9410a41ddbd8b44f69fbf81b399302c0b51.
This new executable has not been submitted by the agent to VirusTotal.

## Change 13 — Exact DPI entry

User could not select exact values such as 1000/2000 with the index slider.
Added explicit numeric DPI entry and presets 1000/2000/3000/4000. Draft values
still require Apply; unsupported values disable Apply and explain the restriction.
Files: src/gui.rs. Hardware protocol and device settings unchanged by this fix.
Checks: two GUI state/profile tests passed; all-target GUI Clippy -D warnings passed.
No additional physical writes: user clarified this was a value-selection problem.

## Change 14 — Explicit RGB effects

Added temporal two-color gradient, breathing and rainbow; 4–30 second cycle,
explicit start/stop. Shared pure color math prepares future device synchronization.
No new HID command. Worker serializes frames using measured Harpoon stage-RGB
setter, fresh DPI preservation and full readback. Wait 250ms after each completed
frame; no catch-up loop. Physical stage change, error, manual apply or selection
stops the effect. No animation auto-start/reconnect or flash/profile writes.
Checks: 43 library + 4 CLI tests passed; all-target GUI Clippy -D warnings passed;
release GUI build passed. Mock tests cover explicit start, stop/no future frames,
error cancellation and physical-stage binding. Physical worker test: 12 gradient
frames read back successfully; stopped with no frame for 700ms; original 1200 DPI
and RGB [106,205,181] restored. This was the user's CURRENT state at test time.
Limits: single-zone temporal gradient; conservative update rate (~2 frames/sec on
this mouse), no spatial wave or cross-device sync. Other mice require their adapter.
