"""Build recognition-only mouse identities. No packet or write permissions."""
import json
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parents[1]
catalog = json.loads((root / "research/device-catalog.json").read_text(encoding="utf-8"))
rows = []
for record in catalog["records"]:
    upstream = record["upstream"]
    if upstream == "libratbag" and record["category"] == "mouse":
        family = record["driver_or_controller"]
    elif upstream == "openrazer" and record["category"] == "mouse":
        family = "razer_model_specific"
    elif upstream == "ckb-next" and record["category"] == "mice":
        family = "corsair_generation_specific"
    elif upstream == "OpenRGB" and record["category"] == "Mouse":
        family = "rgb_only:" + record["driver_or_controller"]
    else:
        continue
    for identity in record["identifiers"]:
        # No receiver-route inference or Bluetooth equivalence from USB identity.
        if identity.get("bus", "").lower() != "usb":
            continue
        if not identity.get("vid") or not identity.get("pid"):
            continue
        rows.append({"vid": int(identity["vid"], 16), "pid": int(identity["pid"], 16),
                     "name": record["name"], "family": family,
                     "source_record": record["record_id"], "source_url": record["source_url"],
                     "identity_kind": identity.get("kind", "upstream_usb_declaration")})
rows.sort(key=lambda r: (r["vid"], r["pid"], r["family"], r["source_record"]))
out = {"schema_version": 1, "checked_on": catalog["checked_on"],
       "purpose": "recognition_only_never_authorizes_io", "identities": rows}
(root / "research/mouse-protocol-index.json").write_text(
    json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
summary = {"identity_source_records": len(rows), "distinct_vid_pid_pairs": len({(r["vid"], r["pid"]) for r in rows}),
           "family_records": dict(sorted(Counter(r["family"] for r in rows).items())),
           "market_coverage_probability": None,
           "note": "Counts are a research inventory, not hardware support or market share."}
(root / "research/mouse-protocol-priorities.json").write_text(
    json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps(summary, ensure_ascii=False, indent=2))
