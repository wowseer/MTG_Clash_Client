# forge-bridge

Java-Brücke zwischen einer Oberfläche (MTG Clash oder eine eigene) und der Forge-Regel-Engine.
Lizenz: **GPL-3.0** (siehe `LICENSE`), da sie Forge-Klassen verwendet.

| Paket | Rolle |
|---|---|
| `clash.forgebridge` | **Öffentliche Schnittstelle**: `ClashBridge` (einbetten, ferngesteuerter Sitz, eigenständiger Betrieb), `TableLauncher` (Tisch-Prozess starten und per Socket verbinden) |
| `clash.forgebridge.engine` | intern: Forge-Adapter (`RemotePlayerController`, `Decisions`, `StateJson`, `BridgeCore` …) |
| `clash.forgebridge.app` | `BridgeMain`: eigenständiger Prozess (stdin/stdout), braucht `forge-gui-desktop` |

- Bauen: `./build.sh` → `build/forge-bridge-core.jar` (Bibliothek zum Einbetten) und
  `build/forge-bridge.jar` (Programm, Main-Class `clash.forgebridge.app.BridgeMain`);
  Forge-Jar über `FORGE_JAR`
- Von Hand starten: `./run.sh` (Forge-Ordner über `FORGE_DIR`, sonst `forge/` hier im Ordner, sonst `~/Games/Forge`)
- Testdecks: `testdecks/`

Protokoll und Einrichtung: [`../docs/Forge-Schnittstelle.md`](../docs/Forge-Schnittstelle.md),
Einbau in andere Projekte: [`../docs/Einbau.md`](../docs/Einbau.md).
