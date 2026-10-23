"""Check research provenance and extraction invariants, entirely offline."""
import csv
import hashlib
import json
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parents[1]
doc = json.loads((root / "research/device-catalog.json").read_text(encoding="utf-8"))
rows = doc["records"]
indexed = {row["record_id"]: row for row in rows}
assert len(indexed) == len(rows), "Duplicate source record ids"
assert doc["purpose"] == "research_only_no_runtime_authorization"
for row in rows:
    assert row["execution_permitted"] is False
    assert row["unidevice_runtime_support"] == "not_asserted"
    assert row["source_url"].startswith("https://")
    serialized = json.dumps(row).lower()
    assert "c:\\\\users\\\\tecno" not in serialized
    assert "c:\\\\unidevice" not in serialized, "Private path in public research"
    for identity in row["identifiers"]:
        for field in ("vid", "pid"):
            value = identity.get(field)
            assert not value or (len(value) == 4 and int(value, 16) >= 0)

# Real inherited metadata: PID override must preserve parent's limits and methods.
lancehead = indexed["openrazer:RazerLanceheadWirelessReceiver"]
assert lancehead["identifiers"][0]["pid"] == "006F"
assert lancehead["parameters"]["dpi_max"] == 16000
assert "set_charge_colour" in lancehead["parameters"]["writable_methods"]
assert "set_dpi_xy" in lancehead["parameters"]["writable_methods"]
assert indexed["openrazer:RazerViperMini"]["parameters"]["dpi_max"] == 8500
assert indexed["openrazer:RazerDeathAdderV2"]["parameters"]["dpi_max"] == 20000
superlight = [r for r in rows if r["upstream"] == "libratbag" and "Superlight" in r["name"]]
assert superlight and all(not r["parameters"] for r in superlight), "Do not infer driver-wide features for Superlight"
harpoon = indexed["ckb-next:P_HARPOON_PRO"]
assert harpoon["identifiers"][0]["pid"] == "1B75"
assert harpoon["parameters"]["dpi_max"] == 12000

# Missing Corsair limits must remain unknown, not upstream GUI's 12000 fallback.
ckb = [r for r in rows if r["upstream"] == "ckb-next"]
assert any(not r["parameters"] for r in ckb)
official = [r for r in rows if r["upstream"] == "manufacturer_docs"]
wired = next(r for r in official if r["name"] == "MSI CLUTCH GM51 LIGHTWEIGHT (wired)")
wireless = next(r for r in official if r["name"] == "MSI CLUTCH GM51 LIGHTWEIGHT WIRELESS")
assert wired["parameters"]["specifications_not_all_adjustable"]["report_rate_max_hz"] == 8000
assert wireless["parameters"]["specifications_not_all_adjustable"]["report_rate_max_hz"] == 1000
snapshot = root / "research/source-data/openrgb-pipeline.csv"
assert hashlib.sha256(snapshot.read_bytes()).hexdigest() == doc["sources"]["OpenRGB"]["table_sha256"]
with snapshot.open(encoding="utf-8-sig", newline="") as f:
    raw = list(csv.DictReader(f))
for row in rows:
    if row["upstream"] != "OpenRGB":
        continue
    original = raw[row["raw_metadata"]["table_row"] - 1]
    assert original["Name"] == row["name"]
    for key, column in [("rgb_direct", "Direct"), ("rgb_effects", "Effects"), ("rgb_persistent_save", "Save")]:
        assert row["parameters"][key]["raw"] == original[column]
with (root / "research/device-catalog.csv").open(encoding="utf-8-sig", newline="") as f:
    exported = list(csv.DictReader(f))
assert len(exported) == len(rows)
assert all(r["runtime_authorized"] == "false" for r in exported)
page = (root / "research/catalog.html").read_text(encoding="utf-8")
assert page.count("<tr>") == len(rows) + 1
stats = json.loads((root / "research/coverage.json").read_text(encoding="utf-8"))
assert stats["source_records"] == len(rows)
assert stats["by_upstream"] == dict(Counter(r["upstream"] for r in rows))
print(f"PASS: {len(rows)} source records, ids, inherited methods, limits, CSV/HTML, source hash and no runtime permissions")
