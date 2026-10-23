"""Extract read-query routing facts, not driver code, from pinned OpenRazer source."""
import json
import re
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
checkout = root / "local/research/openrazer"
commit = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True).strip()
expected = "477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d"
if commit != expected:
    raise RuntimeError("Review extraction against the new upstream revision before updating profiles")
subprocess.run(["git", "-C", str(checkout), "diff", "--exit-code", "HEAD", "--",
                "driver/razermouse_driver.c", "driver/razermouse_driver.h"],
               check=True, stdout=subprocess.DEVNULL)
text = (checkout / "driver/razermouse_driver.c").read_text(encoding="utf-8")
text = re.sub(r"/\*.*?\*/|//[^\n]*", "", text, flags=re.S)
header = (checkout / "driver/razermouse_driver.h").read_text(encoding="utf-8")
constants = {name: int(value, 0) for name, value in re.findall(r"^#define\s+(\w+)\s+(0x[0-9a-fA-F]+|\d+)\b", header, re.M)}


def body(name):
    match = re.search(r"\b" + name + r"\s*\([^)]*\)\s*\{", text)
    if not match:
        raise RuntimeError(name)
    begin = match.end()
    depth = 1
    for i in range(begin, len(text)):
        depth += (text[i] == "{") - (text[i] == "}")
        if not depth:
            return text[begin:i]
    raise RuntimeError("Unbalanced C function")


def groups(content):
    pattern = r"((?:case\s+USB_DEVICE_ID_RAZER_\w+\s*:\s*)+)(.*?)(?=case\s+USB_DEVICE_ID_RAZER_\w+\s*:|default\s*:|$)"
    for match in re.finditer(pattern, content, re.S):
        yield re.findall(r"case\s+(\w+)\s*:", match[1]), match[2]


firmware = {}
for cases, statements in groups(body("razer_attr_read_firmware_version")):
    match = re.search(r"request.transaction_id.id\s*=\s*(0x[0-9a-fA-F]+)", statements)
    if match:
        for symbol in cases:
            firmware[symbol] = int(match[1], 16)

dpi = {}
for cases, statements in groups(body("razer_attr_read_dpi")):
    if not re.search(r"request\s*=\s*razer_chroma_misc_get_dpi_xy\(NOSTORE\)", statements):
        continue
    match = re.search(r"request.transaction_id.id\s*=\s*(0x[0-9a-fA-F]+)", statements)
    if not match:
        raise RuntimeError("DPI transaction not explicit")
    for symbol in cases:
        dpi[symbol] = int(match[1], 16)

routes = {}
for cases, statements in groups(body("razer_get_report")):
    wait = re.search(r"response,\s*(RAZER_\w+_WAIT_US)\)", statements)
    if not wait:
        raise RuntimeError("Unknown read query routing")
    interface = re.search(r"index\s*=\s*(0x[0-9a-fA-F]+|\d+)", statements)
    for symbol in cases:
        routes[symbol] = (int(interface[1], 0) if interface else 0, constants[wait[1]])

catalog = json.loads((root / "research/device-catalog.json").read_text(encoding="utf-8"))
metadata = {}
for row in catalog["records"]:
    if row["upstream"] == "openrazer" and row["category"] == "mouse":
        for identity in row["identifiers"]:
            pid = int(identity["pid"], 16)
            if pid in metadata:
                raise RuntimeError("Ambiguous model metadata")
            metadata[pid] = row

profiles = []
excluded = []
for symbol, transaction in sorted(dpi.items()):
    if "BLUETOOTH" in symbol or symbol not in firmware:
        excluded.append(symbol)
        continue
    pid = constants[symbol]
    model = metadata.get(pid)
    maximum = model["parameters"].get("dpi_max") if model else None
    if not maximum or "get_dpi_xy" not in model["raw_metadata"]["methods"]:
        excluded.append(symbol)
        continue
    interface, wait = routes.get(symbol, (0, constants["RAZER_MOUSE_WAIT_US"]))
    profiles.append({"pid": pid, "name": model["name"], "symbol": symbol,
                     "firmware_transaction": firmware[symbol], "dpi_transaction": transaction,
                     "usb_interface": interface, "minimum_wait_us": wait,
                     "source_declared_max_dpi": maximum, "source_record": model["record_id"]})
profiles.sort(key=lambda p: p["pid"])
assert len({p["pid"] for p in profiles}) == len(profiles)
assert next(p for p in profiles if p["pid"] == 0x0084)["dpi_transaction"] == 0x3F
assert next(p for p in profiles if p["pid"] == 0x008A)["dpi_transaction"] == 0xFF
assert next(p for p in profiles if p["pid"] == 0x0099)["usb_interface"] == 3
output = {"schema_version": 1, "purpose": "experimental_read_only_no_setters", "upstream_commit": commit,
          "source": f"https://github.com/openrazer/openrazer/blob/{commit}/driver/razermouse_driver.c",
          "profiles": profiles, "excluded_symbols": excluded}
(root / "research/razer-read-profiles.json").write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print(f"{len(profiles)} USB model/route profiles; firmware and current XY DPI queries only; {len(excluded)} explicit exclusions")
