"""Offline, static research extraction. Never imports upstream code or opens HID.

Inputs: shallow upstream checkouts and OpenRGB CSV under local/research.
Outputs are documentation, deliberately separate from runtime compatibility packs.
"""
import ast
import configparser
import csv
import hashlib
import html
import json
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INPUT = ROOT / "local/research"
OUT = ROOT / "research"
DATE = "2026-10-23"
REPOS = {
    "libratbag": "https://github.com/libratbag/libratbag",
    "openrazer": "https://github.com/openrazer/openrazer",
    "ckb-next": "https://github.com/ckb-next/ckb-next",
    "OpenRGB": "https://github.com/CalcProgrammer1/OpenRGB",
}
VENDORS = {
    "046D": "Logitech", "1532": "Razer", "1B1C": "Corsair",
    "1038": "SteelSeries", "0951": "HyperX/Kingston", "03F0": "HP/HyperX",
    "0B05": "ASUS", "1E7D": "ROCCAT/Turtle Beach", "1462": "MSI",
    "2516": "Cooler Master", "258A": "OEM/shared VID", "09DA": "A4Tech/Bloody",
    "3434": "Keychron", "31E3": "Wooting",
}
records = []
manifest = {}


def revision(repo):
    return subprocess.check_output(
        ["git", "-C", str(INPUT / repo), "rev-parse", "HEAD"], text=True
    ).strip()


def source(repo, path, line=None):
    suffix = f"#L{line}" if line else ""
    return f"{REPOS[repo]}/blob/{manifest[repo]['commit']}/{path}{suffix}"


def vendor(name, vid=""):
    for pattern, label in [
        ("logitech", "Logitech"), ("razer", "Razer"), ("corsair", "Corsair"),
        ("steelseries", "SteelSeries"), ("hyperx", "HyperX"),
        ("asus", "ASUS"), ("roccat", "ROCCAT/Turtle Beach"),
        ("turtle beach", "ROCCAT/Turtle Beach"), ("msi ", "MSI"),
        ("cooler master", "Cooler Master"), ("coolermaster", "Cooler Master"),
        ("glorious", "Glorious"), ("keychron", "Keychron"), ("wooting", "Wooting"),
        ("gembird", "Gembird"), ("bloody", "A4Tech/Bloody"),
    ]:
        if pattern in name.lower():
            return label
    return VENDORS.get(vid.upper(), "Other/unknown")


def add(repo, key, name, category, ids, driver, parameters, url, notes=None, raw=None):
    records.append({
        "record_id": f"{repo}:{key}", "manufacturer": vendor(name, ids[0].get("vid", "") if ids else ""),
        "name": name, "category": category, "identifiers": ids,
        "upstream": repo, "driver_or_controller": driver, "parameters": parameters,
        "evidence": "upstream_source_declared", "source_url": url,
        "checked_on": DATE, "unidevice_runtime_support": "not_asserted",
        "execution_permitted": False, "notes": notes or [], "raw_metadata": raw or {},
    })


def libratbag():
    for path in sorted((INPUT / "libratbag/data/devices").glob("*.device")):
        cfg = configparser.ConfigParser(interpolation=None, strict=True)
        cfg.optionxform = str
        cfg.read(path, encoding="utf-8")
        device = dict(cfg["Device"])
        sections = {s: dict(cfg[s]) for s in cfg.sections() if s != "Device"}
        params = {}
        # Only explicit device-file declarations; driver support is not a per-model promise.
        fields = {"DpiRange": "dpi_range", "DpiList": "dpi_values", "Dpis": "dpi_slots",
                  "Buttons": "button_count", "Leds": "led_count", "Profiles": "profile_count",
                  "LedType": "led_pin_order", "MacroLength": "macro_length"}
        for section, values in sections.items():
            for k, normalized in fields.items():
                if k in values:
                    params.setdefault(normalized, []).append({"value": values[k], "condition": section})
        ids = []
        for match in device.get("DeviceMatch", "").split(";"):
            if not match:
                continue
            bus, vid, pid = match.split(":")
            ids.append({"bus": bus, "vid": vid.upper(), "pid": pid.upper(), "kind": "upstream_match_not_guaranteed_physical_usb_id"})
        add("libratbag", path.stem, device["Name"], device.get("DeviceType", "unknown"),
            ids, device["Driver"], params, source("libratbag", f"data/devices/{path.name}"),
            ["Missing fields require driver/runtime inquiry; they are not unsupported.",
             "Receiver route, firmware and descriptor still need validation."], {"device": device, "driver_sections": sections})


class Unresolved(Exception):
    pass


def openrazer():
    base = INPUT / "openrazer/daemon/openrazer_daemon/hardware"
    classes = {}
    aliases = {}
    for path in sorted(base.glob("*.py")):
        for node in ast.parse(path.read_text(encoding="utf-8")).body:
            if isinstance(node, ast.ImportFrom):
                for item in node.names:
                    if item.name in {"RazerDevice", "RazerDeviceBrightnessSuspend"}:
                        aliases[item.asname or item.name] = item.name
            if isinstance(node, ast.ClassDef):
                classes[node.name] = (node, path)
    resolved = {}
    warnings = []
    keep = {"USB_VID", "USB_PID", "METHODS", "DPI_MAX", "POLL_RATES", "MATRIX_DIMS", "HAS_MATRIX", "WAVE_DIRS"}

    def expression(node):
        if isinstance(node, ast.Constant):
            return node.value
        if isinstance(node, (ast.List, ast.Tuple)):
            return [expression(x) for x in node.elts]
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            return expression(node.left) + expression(node.right)
        if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
            attrs = resolve(node.value.id)
            if node.attr in attrs:
                return attrs[node.attr]
        raise Unresolved(ast.dump(node))

    def resolve(name):
        name = aliases.get(name, name)
        if name in resolved:
            return resolved[name]
        if name not in classes:
            return {}  # External service base, no device metadata inferred.
        node, path = classes[name]
        attrs = {}
        for parent in reversed(node.bases):
            if isinstance(parent, ast.Name):
                attrs.update(resolve(parent.id))
        for stmt in node.body:
            if isinstance(stmt, ast.Assign):
                for target in stmt.targets:
                    if isinstance(target, ast.Name) and target.id in keep:
                        try:
                            attrs[target.id] = expression(stmt.value)
                        except Unresolved as e:
                            # Do not fall back to a parent's possibly wrong value.
                            attrs.pop(target.id, None)
                            warnings.append(f"{path.name}:{stmt.lineno} {target.id}: {e}")
        resolved[name] = attrs
        return attrs

    for name, (node, path) in classes.items():
        attrs = resolve(name)
        if not isinstance(attrs.get("USB_PID"), int) or not isinstance(attrs.get("USB_VID"), int):
            continue
        methods = attrs.get("METHODS", [])
        params = {"writable_methods": sorted(m for m in methods if m.startswith("set_") or "_set_" in m),
                  "readable_methods": sorted(m for m in methods if m.startswith("get_") or "_get_" in m)}
        for key, normalized in [("DPI_MAX", "dpi_max"), ("POLL_RATES", "polling_hz"), ("MATRIX_DIMS", "led_matrix")]:
            if attrs.get(key) is not None:
                params[normalized] = attrs[key]
        if "set_poll_rate" in methods and not attrs.get("POLL_RATES"):
            params["polling_hz"] = [125, 500, 1000]
            params["polling_origin"] = "daemon device_base.py default, not independent firmware evidence"
        doc = ast.get_docstring(node) or name
        title = re.sub(r"^Class for (the )?", "", doc.strip().splitlines()[0])
        add("openrazer", name, title, path.stem, [{"bus": "usb", "vid": f"{attrs['USB_VID']:04X}", "pid": f"{attrs['USB_PID']:04X}"}],
            "openrazer", params, source("openrazer", f"daemon/openrazer_daemon/hardware/{path.name}", node.lineno),
            ["Class inheritance resolved statically; no upstream Python executed.",
             "Methods are exposed by upstream daemon; availability/side effects need hardware verification."],
            {"class": name, "methods": methods})
    if warnings:
        raise RuntimeError("Unresolved OpenRazer declarations: " + "\n".join(warnings))


def ckb():
    header = (INPUT / "ckb-next/src/daemon/usb.h").read_text(encoding="utf-8")
    body = (INPUT / "ckb-next/src/daemon/usb.c").read_text(encoding="utf-8")
    constants = dict(re.findall(r"^#define\s+(P_\w+)\s+(0x[0-9a-fA-F]+)\b", header, re.M))
    dpi_block = body.split("const dpi_list mouse_dpi_list[] = {", 1)[1].split("};", 1)[0]
    dpi = {key: int(value) for key, value in re.findall(r"\{\s*(P_\w+),\s*(\d+)\s*\}", dpi_block)}
    models_block = body.split("const device_desc models[] = {", 1)[1].split("};", 1)[0]
    category = "unknown"
    for line in models_block.splitlines():
        if "//" in line:
            category = line.split("//", 1)[1].strip().lower()
        match = re.search(r"\{\s*V_CORSAIR,\s*(P_\w+)", line)
        if not match:
            continue
        key = match[1]
        if key not in constants:
            raise RuntimeError(f"Missing Corsair constant {key}")
        params = {"dpi_max": dpi[key]} if key in dpi else {}
        add("ckb-next", key, "Corsair " + key[2:].replace("_", " "), category,
            [{"bus": "usb", "vid": "1B1C", "pid": f"{int(constants[key], 16):04X}"}],
            "ckb-next (registered model, feature subset unresolved)", params,
            source("ckb-next", "src/daemon/usb.c"),
            ["Name derived from C symbol; exact retail name/revision needs reconciliation.",
             "Registration alone does not prove all functions or release support.",
             "No fallback DPI limit inferred when absent from explicit table."], {"symbol": key})


def openrgb():
    path = OUT / "source-data/openrgb-pipeline.csv"
    wanted = {"Mouse", "Keyboard", "Mousemat", "Headset", "HeadsetStand", "Microphone", "Gamepad", "Speaker", "Keyboard/Controller", "DEVICE_TYPE_KEYBOARD"}
    with path.open(encoding="utf-8-sig", newline="") as stream:
        rows = list(csv.DictReader(stream))
    manifest["OpenRGB"]["table_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    manifest["OpenRGB"]["table_url"] = "https://openrgb.org/data/supported_devices_pipeline.csv"
    manifest["OpenRGB"]["table_version"] = "unreleased pipeline; NOT guaranteed same commit as GitHub mirror"
    manifest["OpenRGB"]["all_table_records"] = len(rows)
    status = {"✔️": "declared_supported", "❌": "not_applicable", "⚒️": "partial", "🚨": "problematic", "🚫": "not_supported", "🤖": "automatic_cannot_disable"}
    for i, row in enumerate(rows, 1):
        if not wanted.intersection(row["Category"].splitlines()):
            continue
        if not row["Name"].strip():
            continue  # Controller-only rows are not concrete models.
        params = {key: {"status": status.get(row[col], "unknown"), "raw": row[col]}
                  for key, col in [("rgb_direct", "Direct"), ("rgb_effects", "Effects"), ("rgb_persistent_save", "Save")]}
        add("OpenRGB", str(i), row["Name"], row["Category"],
            [{"bus": row["Type"], **{key.lower(): row[key] for key in ["VID", "PID", "SVID", "SPID"] if row[key]}}],
            row["RGBController"], params, "https://openrgb.org/devices_pipeline.html",
            ["Categories and feature flags may be shared controller metadata, not exact model tests.",
             "Pipeline table is not a stable-release promise; it is not bound to the source checkout commit.",
             "OpenRGB lighting evidence says nothing about DPI/button configuration."],
            {"comments": row["Comments"], "table_row": i})


def flatten(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def manufacturer_models():
    path = OUT / "vendor-models.json"
    doc = json.loads(path.read_text(encoding="utf-8"))
    manifest["manufacturer_docs"] = {"checked_on": doc["checked_on"], "input": "research/vendor-models.json",
                                      "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    for i, row in enumerate(doc["models"], 1):
        add("manufacturer_docs", str(i), row["name"], row["category"], [], row["software"],
            {"documented_controls": row["controls"], "specifications_not_all_adjustable": row["specifications"]},
            row["sources"][0], row["caveats"], {"all_sources": row["sources"]})
        records[-1]["manufacturer"] = row["manufacturer"]
        records[-1]["evidence"] = "manufacturer_documented_not_protocol_verified"


def write_outputs():
    OUT.mkdir(exist_ok=True)
    catalog = {"schema_version": 1, "checked_on": DATE, "purpose": "research_only_no_runtime_authorization",
               "count_semantics": "source records; model/connection variants and overlaps are NOT deduplicated",
               "sources": manifest, "records": records}
    (OUT / "device-catalog.json").write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    with (OUT / "device-catalog.csv").open("w", encoding="utf-8-sig", newline="") as stream:
        writer = csv.writer(stream)
        writer.writerow(["record_id", "manufacturer", "model_or_variant", "category", "identifiers", "upstream", "driver", "parameters", "notes", "source_url", "runtime_authorized"])
        for row in records:
            values = [row["record_id"], row["manufacturer"], row["name"], row["category"], flatten(row["identifiers"]), row["upstream"], row["driver_or_controller"], flatten(row["parameters"]), flatten(row["notes"]), row["source_url"], "false"]
            # CSV spreadsheet formula injection protection for untrusted upstream strings.
            writer.writerow(["'" + v if v.startswith(("=", "+", "-", "@", "\t", "\r")) else v for v in values])
    stats = {"checked_on": DATE, "source_records": len(records),
             "by_upstream": dict(sorted(Counter(r["upstream"] for r in records).items())),
             "by_manufacturer": dict(sorted(Counter(r["manufacturer"] for r in records).items())),
             "physical_tests_in_this_research": 0, "runtime_permissions_added": 0}
    (OUT / "coverage.json").write_text(json.dumps(stats, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    # No fetch, dependencies, scripts from upstream or device calls. All imported text escaped.
    rows = []
    for row in records:
        cells = [row["manufacturer"], row["name"], row["upstream"], flatten(row["identifiers"]),
                 flatten(row["parameters"]), " ".join(row["notes"]) + " " + flatten(row["raw_metadata"])]
        rows.append("<tr>" + "".join("<td>" + html.escape(c) + "</td>" for c in cells)
                    + '<td><a target="_blank" rel="noopener noreferrer" href="' + html.escape(row["source_url"], quote=True) + '">Источник</a></td></tr>')
    page = '''<!doctype html><html lang="ru"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'none'">
<title>UniDevice · каталог исследования</title><style>
body{font:15px system-ui;margin:32px;background:#f4f7f8;color:#172c35}h1{font-size:28px}p{max-width:1050px;line-height:1.6}
input,select{font:inherit;padding:12px;border:1px solid #b6cbd0;border-radius:9px;margin:4px}input{width:min(600px,70vw)}
table{border-collapse:collapse;width:100%;background:white;font-size:13px}th,td{text-align:left;padding:12px;border-bottom:1px solid #d9e3e7;vertical-align:top;overflow-wrap:anywhere}th{background:#dfebee;position:sticky;top:0}td:nth-child(5),td:nth-child(6){min-width:260px;max-width:450px}a{color:#006b71}.muted{color:#4a6570}.scroll{overflow:auto}
</style><h1>UniDevice · каталог устройств и параметров</h1>
<p><b>Pet-проект, полностью написанный нейросетью под надзором пользователя.</b><br>
Срез 23 октября 2026. Это исследовательские записи из открытых проектов, а не список работающих устройств UniDevice.
Одна модель может встречаться несколько раз: разные подключения, ревизии и источники. Отсутствующий параметр означает «не выяснено».
Групповые признаки RGB-контроллера требуют проверки конкретной модели. Каталог не разрешает отправку команд.</p>
<input id="q" placeholder="Модель, VID/PID, параметр: Superlight, 1B75, dpi, set_poll_rate…"><select id="src"><option value="">Все источники</option><option>manufacturer_docs</option><option>libratbag</option><option>openrazer</option><option>ckb-next</option><option>OpenRGB</option></select>
<p id="count" class="muted"></p><div class="scroll"><table><thead><tr><th>Производитель</th><th>Модель / вариант</th><th>Источник</th><th>ID</th><th>Параметры</th><th>Оговорки и исходные поля</th><th>Ссылка</th></tr></thead><tbody>__ROWS__</tbody></table></div>
<script>const rows=[...document.querySelectorAll('tbody tr')];const data=rows.map(r=>r.textContent.toLowerCase());function filter(){const q=document.getElementById('q').value.toLowerCase().trim();const s=document.getElementById('src').value;let n=0;rows.forEach((r,i)=>{const show=(!q||data[i].includes(q))&&(!s||r.cells[2].textContent===s);r.hidden=!show;if(show)n++});document.getElementById('count').textContent=`Показано ${n} из ${rows.length} записей источников`;}document.getElementById('q').addEventListener('input',filter);document.getElementById('src').addEventListener('change',filter);filter();</script></html>'''
    (OUT / "catalog.html").write_text(page.replace("__ROWS__", "\n".join(rows)), encoding="utf-8")
    print(json.dumps(stats, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    for repo in REPOS:
        manifest[repo] = {"url": REPOS[repo], "commit": revision(repo), "checked_on": DATE}
    libratbag()
    openrazer()
    ckb()
    openrgb()
    manufacturer_models()
    write_outputs()
