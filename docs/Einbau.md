# Einbau in andere Projekte

MTG Clash besteht aus wiederverwendbaren Bausteinen, die auch ohne den Godot-Tisch funktionieren.
Mit ihnen lässt sich Forges komplette Regel-Engine an eine eigene Oberfläche anbinden (Rust,
Godot, Bevy, Terminal, Web …) oder ein eigener Spieltisch an einen Forge-Gastgeber (NeoForge,
Forge-Desktop, eigene Java-Programme) hängen.

```
 Oberfläche (beliebig)              Protokoll v3                    Forge (Java)
┌──────────────────────┐      JSON Lines, eine Nachricht    ┌───────────────────────────────┐
│ Godot-Tisch (godot/) │      pro Zeile, UTF-8              │ forge-bridge                  │
│ oder eigene UI       │  ◀── request / state / event ───   │  clash.forgebridge            │
│                      │  ─── answer / start_match … ──▶    │   ClashBridge   (Fassade)     │
│ Rust:                │                                    │   TableLauncher (Tisch starten)│
│  clash-protocol      │   Transport:                       │  clash.forgebridge.engine     │
│  clash-forge-client  │   • stdin/stdout (eigener Prozess) │   (intern: Forge-Adapter)     │
└──────────────────────┘   • TCP 127.0.0.1:N (eingebettet)  │  clash.forgebridge.app        │
                                                            │   BridgeMain (eigener Prozess)│
                                                            └───────────────────────────────┘
                                                             Gastgeber-Anbindungen:
                                                             integrations/neoforge/
```

| Baustein | Ort | Lizenz | Abhängigkeiten | Wofür |
|---|---|---|---|---|
| **Protokoll** | [`docs/Forge-Schnittstelle.md`](Forge-Schnittstelle.md) | – | – | Nachrichten, Befehle, `Decision`/`Choice`, Spielzustand |
| **`clash-protocol`** (Rust-Crate) | `rust/crates/clash-protocol/` | Apache-2.0 | `serde_json` | Protokoll-Modell als Rust-Typen, Lesen/Schreiben der JSON-Form, Befehle, Schnittstelle `RulesEngine` |
| **`clash-forge-client`** (Rust-Crate) | `rust/crates/clash-forge-client/` | Apache-2.0 | `clash-protocol`, `serde_json` | Brücke als Java-Prozess starten **oder** mit einem Gastgeber verbinden; Nachrichten ohne Blockieren abholen |
| **Forge-Brücke** (Java) | `forge-bridge/` | GPL-3.0 | Forge-Jars (Forge 2.0.15+), Gson | Übersetzt Forges `PlayerController`-Aufrufe in `Decision`s und zurück |
| **NeoForge-Anbindung** | `integrations/neoforge/` | GPL-3.0 | NeoForge, Forge-Brücke | Hülle `ClashTable` + Patch für NeoForges Klassen |
| **Godot-Tisch** | `godot/`, `rust/src/` | Apache-2.0 | Godot 4.7, gdext, beide Crates | Die 3D-Oberfläche dieses Projekts |

Die Lizenzgrenze liegt am Prozess bzw. Socket: Alles, was Forge-Klassen benutzt, ist GPL-3.0
(`forge-bridge/`, `integrations/`); die Oberfläche und die Rust-Crates sprechen nur das Protokoll
und enthalten keinen Forge-Code.

---

## 1. Eigene Oberfläche in Rust (Forge als eigener Prozess)

`Cargo.toml`:

```toml
[dependencies]
clash-protocol = { path = "…/MTG Clash/rust/crates/clash-protocol" }
clash-forge-client = { path = "…/MTG Clash/rust/crates/clash-forge-client" }
```

Voraussetzungen: Java 17+, eine Forge-Installation (Ordner mit `res/` und
`forge-gui-desktop-*-jar-with-dependencies.jar`) und die gebaute Brücke
(`forge-bridge/build.sh` → `forge-bridge/build/forge-bridge.jar`).

```rust
use clash_forge_client::{ForgeClient, Incoming, LaunchConfig};
use clash_protocol::{command, message_type, Action, Choice, Decision, Request, RulesEngine};

let mut client = ForgeClient::spawn(&LaunchConfig::new("/opt/forge", "/pfad/forge-bridge.jar"))?;
// jedes Frame (oder in einer Schleife):
while let Some(msg) = client.poll() {
    match msg {
        Incoming::Message(m) => match message_type(&m) {
            "ready" => client.send(&command::StartMatch {
                decks: vec!["/abs/a.dck".into(), "/abs/b.dck".into()], // Pfade absolut
                ..Default::default()                                    // Sitz 0 = deine UI
            }.to_json())?,
            "request" => {
                let req = Request::from_json(&m).unwrap();
                // req.decision zeigen, Spieler entscheiden lassen …
                if let Decision::Priority { .. } = req.decision {
                    client.submit(req.id, &Choice::Action(Action::Pass));
                }
            }
            "state" | "event" | "log" | "game_over" => { /* anzeigen */ }
            _ => {}
        },
        Incoming::Invalid(_) => {}
        Incoming::Closed => break,
    }
}
```

Lauffähiges Beispiel (ein Tisch ohne Grafik, der einen Sitz mit einer einfachen Regel spielt):

```sh
cd rust
cargo run -p clash-forge-client --example autoplay -- \
  ../forge-bridge/forge ../forge-bridge/build/forge-bridge.jar \
  ../forge-bridge/testdecks/rot.dck ../forge-bridge/testdecks/gruen.dck
```

Hinweise:
- Jede Nachricht `request` muss mit genau einer Antwort (`answer`) beantwortet werden; bis dahin
  wartet Forge. `Request::labels` liefert Anzeigetexte zu den Einträgen der Entscheidung,
  `Request::cards` die Kartendaten der beteiligten Karten.
- Den vollständigen Spielzustand schickt die Brücke mit jeder Anfrage (`state`) und auf Wunsch
  (`command::request_state`).
- Welche Forge-Frage zu welcher `Decision` wird, steht in
  [`Forge-Schnittstelle.md`](Forge-Schnittstelle.md), Abschnitt „Was die Engine entscheidet“.

## 2. Eigene Oberfläche in einer anderen Sprache

Das Protokoll ist schlichtes JSON Lines. Ohne die Rust-Crates:

```sh
java -Xmx2048m -Dfile.encoding=UTF-8 -Djava.awt.headless=true \
     -cp forge-bridge.jar:forge-gui-desktop-…-jar-with-dependencies.jar \
     clash.forgebridge.app.BridgeMain          # Arbeitsverzeichnis: Forge-Ordner
```

Dann Zeilen auf stdin schreiben (`{"cmd":"ping","nonce":1}`, `{"cmd":"start_match",…}`,
`{"cmd":"answer","id":…,"choice":{…}}`) und Zeilen von stdout lesen. Alle sonstigen Ausgaben von
Forge gehen nach stderr. `forge-bridge/run.sh` startet die Brücke so zum Ausprobieren von Hand.

## 3. Eigenen Tisch an einen Forge-Gastgeber hängen (Java)

Ein Gastgeber ist ein Java-Programm, das Forge schon geladen hat und Partien selbst startet
(NeoForge, Forge-Desktop, ein eigener Server). Er bindet die Bibliothek
`forge-bridge/build/forge-bridge-core.jar` ein (ohne Programmstart, braucht nur `forge-game`,
`forge-ai`, `forge-core`, `forge-gui` und Gson) und benutzt nur zwei Klassen:

```java
import clash.forgebridge.ClashBridge;
import clash.forgebridge.TableLauncher;

// 1. Tisch starten (öffnet 127.0.0.1:N, startet "<godot> --path <projekt> -- --clash-port=N",
//    nimmt die Verbindung an und hängt die Brücke daran)
TableLauncher.Session table = TableLauncher.launch(new TableLauncher.Config(godotExe, projectDir));

// 2. Den Sitz, den der Tisch spielt, als ferngesteuerten Spieler eintragen
RegisteredPlayer human = new RegisteredPlayer(deck);   // Commander: RegisteredPlayer.forCommander(deck)
human.setPlayer(ClashBridge.remotePlayer("Spieler", 0));          // 0 = Sitzplatz-Index

// 3. Partie wie gewohnt starten (HostedMatch / Match); jede Entscheidung dieses Sitzes geht an den Tisch.
// 4. Nach der Partie
table.close();
```

- `ClashBridge.isRemote(lobbyPlayer)`: ist ein Sitz vom Tisch gesteuert (z. B. damit eine
  KI-Absicherung des Gastgebers ihn nicht ersetzt)?
- Schließt der Spieler das Tisch-Fenster mitten in der Partie, geben seine Sitze auf
  (`ClashBridge.concedeRemotePlayers`).
- Eigener Transport statt `TableLauncher`: `ClashBridge.attach(outputStream, inputStream, onClose)`.
- Ein anderer Tisch als Godot muss nur `--clash-port=N` verstehen, sich mit `127.0.0.1:N`
  verbinden und das Protokoll sprechen – in Rust: `ForgeClient::connect(port)` mit
  `clash_forge_client::host_port(std::env::args())`. Das Beispiel `autoplay` tut genau das,
  wenn es mit `--clash-port=N` gestartet wird.

Nur `ClashBridge` und `TableLauncher` sind öffentliche Schnittstelle. `clash.forgebridge.engine`
ist intern und kann sich ändern.

## 4. NeoForge

Fertige Anbindung in [`integrations/neoforge/`](../integrations/neoforge/README.md): die Hülle
`forge.neo.clash.ClashTable` (Einstellungen von NeoForge lesen, `TableLauncher` aufrufen) und ein
Patch für vier NeoForge-Klassen und die Sprachdateien. Einbau in ein NeoForge-Repo:

```sh
git -C <neoforge> apply "<MTG Clash>/integrations/neoforge/neoforge-host.patch"
NEOFORGE=<neoforge> tools/sync_neoforge.sh
```

## 5. Bauen

| Was | Befehl | Ergebnis |
|---|---|---|
| Java-Brücke | `forge-bridge/build.sh` (Forge-Jar über `FORGE_JAR`, sonst `forge-bridge/forge/` bzw. `~/Games/Forge`) | `forge-bridge/build/forge-bridge-core.jar` (Bibliothek), `forge-bridge/build/forge-bridge.jar` (Programm, Main-Class `clash.forgebridge.app.BridgeMain`) |
| Rust-Crates + Godot-Erweiterung | `cd rust && cargo build` | `rust/target/debug/libmtg_clash.so` |
| Nur die Crates testen | `cd rust && cargo test -p clash-protocol -p clash-forge-client` | – |
| NeoForge-Patch-Jar (ohne Maven) | `tools/neoforge_patch.sh` | `forge-bridge/build/neoforge/neoforge-clash-patch.jar` |
| NeoForge-Patch neu erzeugen | `tools/neoforge_export_patch.sh` | `integrations/neoforge/neoforge-host.patch` |
