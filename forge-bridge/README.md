# forge-bridge

Java bridge between a front end (Neo Forge 3D or your own) and the Forge rules engine.
License: **GPL-3.0** (see `LICENSE`), because it uses Forge classes.

| Package | Role |
|---|---|
| `clash.forgebridge` | **Public API**: `ClashBridge` (embedding, remote-controlled seat, standalone mode), `TableLauncher` (start the table process and connect via socket) |
| `clash.forgebridge.engine` | internal: Forge adapter (`RemotePlayerController`, `Decisions`, `StateJson`, `BridgeCore` …) |
| `clash.forgebridge.app` | `BridgeMain`: standalone process (stdin/stdout), needs `forge-gui-desktop` |

- Build: `./build.sh` → `build/forge-bridge-core.jar` (library for embedding) and
  `build/forge-bridge.jar` (program, main class `clash.forgebridge.app.BridgeMain`);
  Forge jar via `FORGE_JAR`
- Run by hand: `./run.sh` (Forge folder via `FORGE_DIR`, otherwise `forge/` in this folder,
  otherwise `~/Games/Forge`), then type JSON lines, e.g. `{"cmd":"ping"}`
- Test decks: `testdecks/`

Protocol and setup: [`../docs/Forge-Protocol.md`](../docs/Forge-Protocol.md),
embedding in other projects: [`../docs/Embedding.md`](../docs/Embedding.md).
