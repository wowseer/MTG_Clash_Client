# MTG Clash

Eine 3D-Oberfläche (Godot 4.7 + Rust) für die komplette Regel-Engine von
[Forge](https://github.com/Card-Forge/forge): Jede Entscheidung, die Forge einem Spieler abverlangt,
trifft er am 3D-Tisch; die KI spielt nur die Gegner. Eigenständig startet der Tisch Forge selbst,
eingebettet lässt ihn [NeoForge](https://github.com/AdrianLopez98/NeoForge) seine Partien spielen.

Die Anbindung an Forge ist in wiederverwendbare Bausteine gekapselt – für eigene Oberflächen und
andere Forge-Programme: **[docs/Einbau.md](docs/Einbau.md)**.

| Ordner | Inhalt | Lizenz |
|---|---|---|
| `godot/` | Godot-Projekt (Hauptszene `table.tscn`) | Apache-2.0 |
| `rust/` | Godot-Erweiterung (gdext): Tisch, HUD, Darstellung | Apache-2.0 |
| `rust/crates/clash-protocol/` | Protokoll-Modell `Decision`/`Choice` + JSON, ohne Godot | Apache-2.0 |
| `rust/crates/clash-forge-client/` | Verbindung zur Brücke (Java-Prozess oder Gastgeber), ohne Godot; Beispiel `autoplay` | Apache-2.0 |
| `forge-bridge/` | Java-Brücke zu Forge: Fassade `ClashBridge`, `TableLauncher`, Forge-Adapter | GPL-3.0 |
| `integrations/neoforge/` | Anbindung an NeoForge: `ClashTable` + Patch für NeoForges Klassen | GPL-3.0 |
| `docs/` | Bedienung (`DOKUMENTATION.md`), Protokoll (`Forge-Schnittstelle.md`), Einbau (`Einbau.md`), API-Referenz | – |
| `tools/` | Skripte (Doku, NeoForge-Sync/-Patch/-Test) | – |

## Schnellstart

```sh
forge-bridge/build.sh            # Java-Brücke (braucht Forge, siehe docs/Forge-Schnittstelle.md)
cd rust && cargo build           # Godot-Erweiterung und Crates
# Godot 4.7 öffnen: godot/project.godot, F5
```

Forge selbst (Jar und Kartendaten) liegt nicht im Repo; die Brücke liest sie aus einer lokalen
Forge-Installation.
