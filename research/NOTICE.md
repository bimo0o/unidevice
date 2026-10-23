# Research provenance and licensing

This directory contains factual model/parameter inventories extracted from public
sources, attribution links, an archived OpenRGB public device table, and original
UniDevice analysis. It is not an implementation of those projects' drivers.
No upstream driver is linked, imported, installed or executed by UniDevice.

- libratbag: https://github.com/libratbag/libratbag — MIT. The upstream copyright
  and permission notice is preserved in `licenses/libratbag-MIT.txt`.
- OpenRazer hardware declarations: https://github.com/openrazer/openrazer —
  SPDX GPL-2.0-or-later on the inspected Python hardware files. See
  `licenses/OpenRazer-GPL-2.0-or-later.txt`. Documentation may have a different
  license (the upstream LICENSES directory also contains CC-BY-SA-4.0).
- ckb-next: https://github.com/ckb-next/ckb-next — GPL v2 family; inspect each
  source notice before code reuse. License text is in `licenses/ckb-next-GPL-2.0.txt`.
- OpenRGB: https://github.com/CalcProgrammer1/OpenRGB and https://openrgb.org —
  GPL v2 family (some source files specify GPL-2.0-or-later). License text is in
  `licenses/OpenRGB-GPL-2.0.txt`. The public device CSV is retained with its exact
  bytes, originating URL and hash. It has not been relabeled as MIT data.

Exact checkout commits, table URL/hash and retrieval date are in
`device-catalog.json -> sources`. Hardware declarations and method names are
reported as technical facts with references; they do not imply permission to
reuse upstream source code under UniDevice's MIT license. The root license does
not relicense upstream material in this research directory.

`razer-read-profiles.json` records technical routing facts from OpenRazer's mouse
driver at commit 477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d: model/PID, transaction
IDs, USB interface, response delay and linked hardware declarations. The extraction
script is original UniDevice analysis. The independent Rust read-query implementation
does not copy or link upstream driver functions. This inventory neither enables
setters nor establishes physical model support. See specs/razer-read-only.md.

Manufacturer model information is paraphrased factual documentation with direct
links in `vendor-models.json` and `README.md`. Product names/trademarks belong to
their owners. Manuals, images and proprietary software binaries are not bundled.
