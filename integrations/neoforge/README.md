# NeoForge integration

Lets NeoForge (JavaFX front end for Forge, github.com/AdrianLopez98/NeoForge) play its matches on
the 3D table of Neo Forge 3D. Menus, decks and modes stay with NeoForge; for the human seat
NeoForge starts Godot as the game table and drives it through the Forge bridge (protocol v3, local
socket). Matches with more than two players keep using NeoForge's own table.

Everything here is kept separate from the bridge itself, so it can be handed to any NeoForge
repository.

## Contents

| File | What |
|---|---|
| `src/forge/neo/clash/ClashTable.java` | Wrapper for NeoForge: reads the settings (system property → environment variable → NeoForge setting), starts the table via `clash.forgebridge.TableLauncher`, provides the remote-controlled player (`ClashBridge.remotePlayer`) |
| `neoforge-host.patch` | Changes to NeoForge's own classes against NeoForge upstream: `NeoGame` (3D table for the human seat, `NeoMatchUI` runs invisibly as a spectator), `SafeAi` (do not replace the remote seat with the AI fallback), `NeoMain` (test command `clash`), `SettingsPanel` (section "3D table (Neo Forge 3D)"), language files |

The bridge itself (`clash.forgebridge` from `forge-bridge/src/`, without the program entry point
`app/`) is copied in by `tools/sync_neoforge.sh`.

## Installing

```sh
NEOFORGE=~/Entwicklung/Java/NeoForge          # NeoForge checkout
git -C "$NEOFORGE" apply integrations/neoforge/neoforge-host.patch   # once
NEOFORGE="$NEOFORGE" tools/sync_neoforge.sh    # copy bridge + ClashTable (after every change)
```

NeoForge then builds with Maven as usual. Without Maven, against an installed NeoForge:
`tools/neoforge_patch.sh` → put `forge-bridge/build/neoforge/neoforge-clash-patch.jar` in front
of the NeoForge jars on the class path (see `tools/neoforge_clash.sh`).

## Enabling

In NeoForge under *Settings → 3D table (Neo Forge 3D)*: enable the table, enter the Godot
executable (Godot 4.7) and the project folder (`godot/` of Neo Forge 3D). Or at startup:

| System property | Environment variable | Meaning |
|---|---|---|
| `neo.clash=true` | `NEOFORGE_CLASH=true` | use the 3D table |
| `neo.clash.godot` | `CLASH_GODOT` | Godot executable (default `godot`) |
| `neo.clash.project` | `CLASH_PROJECT` | Godot project folder of Neo Forge 3D |

If the table does not connect within 2 minutes, NeoForge plays with its own table.

## Updating the patch

When NeoForge's classes change in the repository (committed locally), regenerate the patch:

```sh
tools/neoforge_export_patch.sh      # BASE=origin/HEAD, NEOFORGE=… can be overridden
```

Check that it applies to an unmodified NeoForge:
`git -C <fresh NeoForge checkout> apply --check integrations/neoforge/neoforge-host.patch`.

## License

GPL-3.0, like NeoForge and Forge.
