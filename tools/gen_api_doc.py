#!/usr/bin/env python3
"""Erzeugt docs/API-Referenz.md aus den Rust-Doku-Kommentaren (///) aller Godot-Klassen.

Dieselben Kommentare erscheinen dank des gdext-Features `register-docs` auch in der
Godot-Editor-Hilfe (F1). Aufruf aus dem Projektordner:

    python3 tools/gen_api_doc.py
"""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "rust" / "src"
OUT = ROOT / "docs" / "API-Referenz.md"


def doc_above(lines, i):
    """Doku-Kommentar über Zeile i (Attribute dazwischen werden übersprungen)."""
    k = i - 1
    while k >= 0 and lines[k].strip().startswith("#["):
        k -= 1
    doc = []
    while k >= 0 and lines[k].strip().startswith("///"):
        doc.insert(0, lines[k].strip()[3:].strip())
        k -= 1
    return " ".join(d for d in doc if d) if doc else ""


def cfg_debug_only(lines, i):
    k = i - 1
    while k >= 0 and (lines[k].strip().startswith("#[") or lines[k].strip().startswith("///")):
        if "cfg(debug_assertions)" in lines[k]:
            return True
        k -= 1
    return False


classes = {}  # Name -> dict
order = []

for path in sorted(SRC.rglob("*.rs")):
    lines = path.read_text().split("\n")
    rel = path.relative_to(ROOT)
    current_impl = None
    godot_api = False
    for i, line in enumerate(lines):
        s = line.strip()
        # Klassen
        if s.startswith("#[derive(GodotClass"):
            j = i + 1
            while not lines[j].strip().startswith("pub struct"):
                j += 1
            name = re.search(r"struct (\w+)", lines[j]).group(1)
            base = re.search(r"base=(\w+)", "\n".join(lines[i:j]))
            classes[name] = {
                "file": str(rel), "base": base.group(1) if base else "Object",
                "doc": doc_above(lines, i), "props": [], "funcs": [], "signals": [],
            }
            order.append(name)
            current_struct = name
            # Eigenschaften im Struct
            k = j + 1
            while not lines[k].startswith("}"):
                t = lines[k].strip()
                if (t.startswith("#[export") and not t.startswith("#[export_group")) or t.startswith("#[var"):
                    m = k + 1
                    while lines[m].strip().startswith("#["):
                        m += 1
                    field = re.match(r"(?:pub(?:\(crate\))? )?(\w+): (.+),", lines[m].strip())
                    if field and field.group(1) not in [p[0] for p in classes[name]["props"]]:
                        classes[name]["props"].append((field.group(1), field.group(2), doc_above(lines, k)))
                k += 1
        # impl-Blöcke
        m = re.match(r"impl (\w+) \{", s)
        if m:
            current_impl = m.group(1)
            godot_api = i > 0 and lines[i - 1].strip().startswith("#[godot_api")
        if s == "#[func]" and current_impl in classes:
            j = i + 1
            sig = lines[j].strip()
            while not sig.endswith("{"):
                j += 1
                sig += " " + lines[j].strip()
            fm = re.match(r"(?:pub(?:\(crate\))? )?fn (\w+)\((.*?)\)\s*(?:->\s*(.+?))?\s*\{", sig)
            if fm:
                params = [p.strip() for p in fm.group(2).split(",") if p.strip() and "self" not in p]
                classes[current_impl]["funcs"].append(
                    (fm.group(1), ", ".join(params), (fm.group(3) or "").strip(), doc_above(lines, i), "&self" not in fm.group(2) and "self" not in fm.group(2))
                )
        if s == "#[signal]" and current_impl in classes:
            sm = re.match(r"fn (\w+)\((.*?)\);", lines[i + 1].strip())
            if sm:
                classes[current_impl]["signals"].append((sm.group(1), sm.group(2), doc_above(lines, i)))

ENUMS_AS_INT = {"ManaType": "Manafarbe", "CardKind": "Kartentyp", "ZoneKind": "Zonenart", "CardAbility": "Fähigkeit"}


def godot_type(t: str) -> str:
    """Rust-Typ -> Godot-/GDScript-Schreibweise."""
    t = t.strip()
    m = re.fullmatch(r"Option<Gd<(\w+)>>|Gd<(\w+)>", t)
    if m:
        return m.group(1) or m.group(2)
    m = re.fullmatch(r"Array<Gd<(\w+)>>", t)
    if m:
        return f"Array[{m.group(1)}]"
    if t in ENUMS_AS_INT:
        return f"int ({ENUMS_AS_INT[t]})"
    return {
        "i32": "int", "i64": "int", "u32": "int", "f32": "float", "f64": "float", "bool": "bool",
        "GString": "String", "VarDictionary": "Dictionary", "": "void",
    }.get(t, t)


def godot_params(p: str) -> str:
    out = []
    for part in [x.strip() for x in p.split(",") if x.strip()]:
        name, _, ty = part.partition(":")
        name = name.replace("mut ", "").strip().lstrip("_")
        out.append(f"{name}: {godot_type(ty)}")
    return ", ".join(out)


def md(text: str) -> str:
    """Godot-Doku-Verweise für Markdown lesbar machen."""
    text = re.sub(r"\[method ([\w.]+)\]", r"`\1()`", text)
    text = re.sub(r"\[member ([\w.]+)\]", r"`\1`", text)
    text = re.sub(r"\[([A-Z]\w+)\]", r"`\1`", text)
    return text.replace("|", "\\|")


DEBUG_ONLY = {"DebugPanel"}
out = [
    "# API-Referenz (automatisch erzeugt)",
    "",
    "Alle Godot-Klassen der Rust-Erweiterung mit Eigenschaften, Funktionen und Signalen.",
    "Erzeugt mit `python3 tools/gen_api_doc.py` aus den Rust-Doku-Kommentaren – dieselben",
    "Texte zeigt der Godot-Editor unter **Hilfe → Hilfe durchsuchen (F1)**.",
    "",
    "Sitzplätze werden als Zahl übergeben: `0` = Spieler (unten), `1` = Gegner (oben).",
    "",
    "## Inhalt",
    "",
]
for name in order:
    c = classes[name]
    tag = " *(nur Test-Build)*" if name in DEBUG_ONLY else ""
    out.append(f"- [{name}](#{name.lower()}) – erbt von `{c['base']}`{tag}")
out.append("")
for name in order:
    c = classes[name]
    out += [f"## {name}", "", f"*Erbt von `{c['base']}` · Quelle: `{c['file']}`*", ""]
    if name in DEBUG_ONLY:
        out += ["> Existiert nur in Test-Builds (`cargo build`), nicht in `cargo build --release`.", ""]
    if c["doc"]:
        out += [md(c["doc"]), ""]
    if c["props"]:
        out += ["### Eigenschaften", "", "| Name | Typ | Beschreibung |", "|---|---|---|"]
        for n, t, d in c["props"]:
            out.append(f"| `{n}` | `{godot_type(t)}` | {md(d)} |")
        out.append("")
    if c["funcs"]:
        out += ["### Funktionen", "", "| Funktion | Rückgabe | Beschreibung |", "|---|---|---|"]
        for n, p, r, d, static in c["funcs"]:
            prefix = "static " if static else ""
            out.append(f"| `{prefix}{n}({godot_params(p)})` | `{godot_type(r)}` | {md(d)} |")
        out.append("")
    if c["signals"]:
        out += ["### Signale", "", "| Signal | Beschreibung |", "|---|---|"]
        for n, p, d in c["signals"]:
            out.append(f"| `{n}({godot_params(p)})` | {md(d)} |")
        out.append("")

OUT.write_text("\n".join(out) + "\n")
total = sum(len(c["props"]) + len(c["funcs"]) + len(c["signals"]) for c in classes.values())
missing = sum(1 for c in classes.values() for x in c["props"] + c["funcs"] + c["signals"] if not x[2 if len(x) == 3 else 3])
print(f"{OUT.relative_to(ROOT)}: {len(order)} Klassen, {total} Einträge, ohne Beschreibung: {missing}")
