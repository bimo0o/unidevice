# Unresolved antivirus detection

Date: 2026-10-23. Historical incident record. The original vendor verdict was not independently
resolved. The user subsequently reported no VirusTotal detections and explicitly authorized
continued GUI development and test builds. The distribution hold was lifted for those requested
test builds; no antivirus exclusions or protection changes were made. New builds have new hashes
and have not automatically inherited the user's earlier scan result.

User reported Kaspersky File Anti-Virus deleting a locally compiled object:

- Detection: `VHO:Trojan-Ransom.Win32.Crypren.gen`, heuristic, high severity.
- Object: `target/release/deps/unidevice-bb3de98fa7ce6870.exe`.
- Reported MD5: `64892bc36f16d52c502dd4d53c713db9`.
- Initiating application: `cargo.exe` in the project's isolated Rust toolchain.

This event identifies the generated EXE as the detected object. It does not independently
establish that cargo.exe is infected. The detected object was not recovered, modified or
excluded from protection. Its reported MD5 could not be independently recomputed after deletion.
Release execution attempts failed with sharing violations before the file disappeared.

## Checks performed and their limits

- Reviewed application source: profile loading/creation is the only application filesystem
  access. Saving uses create_new, never overwrites existing files. No recursive filesystem
  traversal, encryption, file deletion, process execution, autorun or networking in `src/`.
  Tests launch the application and remove their own temporary profile only.
- 25 tests and strict Clippy had passed before the user supplied the antivirus event.
  Passing tests do not prove absence of malicious code or a compromised build environment.
- SHA-256 checks of 24 downloaded `.crate` archives match their `Cargo.lock` checksums.
  Evidence: [reports/dependency-integrity.json](reports/dependency-integrity.json).
  Matching a registry checksum is an integrity check, not a malware verdict.
- OSV query for those 24 package versions returned no published vulnerability matches.
  This does not scan binary behavior, detect all supply-chain compromises or prove safety.
- Reviewed process invocations in dependency build scripts: compiler version/probe operations;
  proc-macro2 removes its generated probe directory under OUT_DIR. This was a targeted
  review, not a complete independent audit of all transitive dependency code.
- CI independently built/tested on hosted Windows/Linux runners; both jobs passed at
  source commit `ad4f127` (run https://github.com/bimo0o/unidevice/actions/runs/39873451484).
  Tests, Clippy, release build and offline demo succeeded. A successful run cannot
  overturn the antivirus verdict and must not be used to bypass local protection.

No binary, source sample or message was submitted to Kaspersky or another analysis service.
Public dependency names/versions were queried through OSV; the reported hash was searched
on the web. No private source or binary was uploaded to a scanner.

## Existing artifacts, not approved for distribution

Windows debug SHA-256 (measured before this review):
`56bbe75351e7830dd02af4a3abedb5dfbb35dd505e17b97f25b72c0f0bf7076b`.

Linux MUSL SHA-256:
`cd851cded026c2f5a3ffa93146c1ef56222b5ac178be5e47ca13de14ef2c8d8f`.

Neither different hashes nor a lack of an alert for these files establishes safety.
Packaging now fails explicitly while `packaging/DISTRIBUTION_HOLD` exists. No GitHub release
was published; executable artifact upload was removed from future CI runs. The two initial
private CI binary artifacts were deleted; listing that run's artifacts returned zero.

## Unresolved work

Obtain a vendor reanalysis/verdict through Kaspersky's official channel, then independently
review the build chain and validate the exact artifact intended for release. Do not assume
that changing compiler flags, names or toolchains resolves the underlying issue. Do not
restore a quarantined file merely to run it, add exclusions or disable protection.

Official references:
- https://encyclopedia.kaspersky.com/knowledge/heuristic-and-proactive-detections/
- https://opentip.kaspersky.com/howto/
- https://osv.dev/

## User-authorized inspection rebuild

On 2026-10-23 the user requested another build to inspect through VirusTotal themselves.
Rebuilt source `f039087` with the same release settings using offline locked dependencies.
The 412,672-byte EXE remained readable when checked; it was not executed or uploaded.
SHA-256: `e4d9b42caeed1850f3f2e0010c946b0e902eb3a47452473e56b13eefad39f88e`.
MD5: `193fc7645db46ec393d31a7a90561a3c`.
This is a new sample, not the deleted object with reported MD5 64892bc36f16d52c502dd4d53c713db9.
The distribution hold and unresolved security status remain in force.
