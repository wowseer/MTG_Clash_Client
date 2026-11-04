//! `ForgeBridge`: startet die Java-Brücke zu Forge und übersetzt das JSON-Lines-Protokoll
//! in Godot-Signale und -Methoden.
//!
//! - Ein Lesethread liest stdout des Java-Prozesses zeilenweise und schickt die Zeilen über
//!   einen Kanal an den Hauptthread; `process()` parst sie (Godot-`JSON`) und sendet Signale.
//! - Befehle werden als eine JSON-Zeile auf stdin geschrieben.
//! - stderr (Forge-Logausgaben) landet in `user://forge-bridge.log`.
//! - Eingebettet (Godot von NeoForge gestartet mit `-- --clash-port=N`): statt des Java-Prozesses
//!   eine TCP-Verbindung zu `127.0.0.1:N`, gleiches Protokoll; die Partie startet der Gastgeber.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::net::TcpStream;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use godot::classes::{INode, Json, Node, ProjectSettings};
use godot::prelude::*;

use crate::rules::{Choice, PlayerId, RulesEngine};

/// Höchstens so viele Nachrichten werden pro Frame verarbeitet (Forge sendet in Spitzen
/// Hunderte Ereignisse auf einmal).
const MAX_MESSAGES_PER_FRAME: usize = 400;
/// Eingebettet: so lange (Sekunden) bleibt der Tisch nach dem Ende der Verbindung offen.
const HOSTED_CLOSE_DELAY: f64 = 4.0;

enum Incoming {
    Line(String),
    Closed,
}

/// Verbindung zur Forge-Regel-Engine: startet die Java-Brücke (`forge-bridge/`) als eigenen
/// Prozess und spricht mit ihr über JSON-Zeilen (Protokoll: `docs/Forge-Schnittstelle.md`).
///
/// Ablauf: [method start_bridge] → Signal [signal bridge_ready] → [method start_match] →
/// Signale [signal forge_event] (jedes Spielereignis) und [signal decision_requested]
/// (ferngesteuerter Spieler hat Priorität / Mulligan) → Antwort mit [method answer] →
/// [signal game_over]. Ferngesteuert sind die Sitzplätze aus `remote_seats`; alle anderen
/// spielt die Forge-KI.
///
/// Zahlen in empfangenen Dictionaries sind ganzzahlig (int), wenn sie keinen Nachkommaanteil
/// haben (Godots JSON liefert sonst immer float).
#[derive(GodotClass)]
#[class(base=Node)]
pub struct ForgeBridge {
    /// Java-Programm (Name im PATH oder absoluter Pfad). Java 17 oder neuer.
    #[export]
    java_path: GString,
    /// Forge-Installationsordner (enthält `res/`). Leer = Umgebungsvariable `FORGE_DIR`,
    /// sonst die Projektkopie `forge-bridge/forge/` (falls vorhanden), sonst `~/Games/Forge`.
    #[export(global_dir)]
    forge_dir: GString,
    /// Forge-Jar mit allen Abhängigkeiten. Leer = erste Datei
    /// `forge-gui-desktop-*-jar-with-dependencies.jar` in [member forge_dir].
    #[export(global_file = "*.jar")]
    forge_jar: GString,
    /// Gebaute Brücke (`forge-bridge/build.sh`). `res://`-Pfade werden umgerechnet.
    #[export]
    bridge_jar: GString,
    /// Maximaler Java-Heap in MB (`-Xmx`).
    #[export(range = (256.0, 16384.0))]
    java_max_memory_mb: i32,
    /// Brücke automatisch in `ready()` starten.
    #[export]
    autostart: bool,
    /// Jede empfangene Nachricht in die Godot-Konsole schreiben (zum Debuggen).
    #[export]
    log_messages: bool,

    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// Eingebettet: Verbindung zum Gastgeber (NeoForge), der die Partie startet.
    socket: Option<TcpStream>,
    rx: Option<Receiver<Incoming>>,
    ready: bool,
    base: Base<Node>,
}

#[godot_api]
impl INode for ForgeBridge {
    fn init(base: Base<Node>) -> Self {
        Self {
            java_path: "java".into(),
            forge_dir: GString::new(),
            forge_jar: GString::new(),
            bridge_jar: "res://../forge-bridge/build/forge-bridge.jar".into(),
            java_max_memory_mb: 2048,
            autostart: false,
            log_messages: false,
            child: None,
            stdin: None,
            socket: None,
            rx: None,
            ready: false,
            base,
        }
    }

    fn ready(&mut self) {
        if self.autostart {
            self.start_bridge();
        }
    }

    fn process(&mut self, _delta: f64) {
        let mut lines = Vec::new();
        let mut closed = false;
        if let Some(rx) = &self.rx {
            while lines.len() < MAX_MESSAGES_PER_FRAME {
                match rx.try_recv() {
                    Ok(Incoming::Line(l)) => lines.push(l),
                    Ok(Incoming::Closed) | Err(TryRecvError::Disconnected) => {
                        closed = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }
        for line in lines {
            self.dispatch(&line);
        }
        if closed {
            self.on_closed();
        }
    }

    fn exit_tree(&mut self) {
        self.stop_bridge();
    }
}

#[godot_api]
impl ForgeBridge {
    /// Die Brücke ist gestartet und Forge geladen; `protocol` = Protokollversion.
    #[signal]
    fn bridge_ready(protocol: i64);

    /// Jede empfangene Nachricht (vor den speziellen Signalen), z. B. für eigene Auswertung.
    #[signal]
    fn message_received(message: VarDictionary);

    /// Ein Forge-Spielereignis: `{event, text, turn, phase}` (`event` = Forge-Klassenname,
    /// z. B. "GameEventCardChangeZone").
    #[signal]
    fn forge_event(event: VarDictionary);

    /// Neuer Eintrag im Spielprotokoll der Engine: `{kind, text, card, turn}` (`kind` z. B.
    /// "ZONE_CHANGE", "DAMAGE", "STACK_ADD", "TURN"; `card` = Karten-ID der Quelle, falls bekannt).
    #[signal]
    fn log_entry(entry: VarDictionary);

    /// Ein ferngesteuerter Spieler muss entscheiden (Protokoll v3): `{id, player, decision, prompt,
    /// source, ability, cancelable, labels, state}`; `decision` ist eine `Decision`
    /// (z. B. `{"type": "Priority", "actions": […]}`). Antwort mit [method answer] und einer
    /// `Choice`. Aufbau siehe `docs/Forge-Schnittstelle.md`.
    #[signal]
    fn decision_requested(request: VarDictionary);

    /// Die gewählte Option wurde von Forge abgelehnt (`{player, option, reason}`);
    /// es folgt eine neue Anfrage.
    #[signal]
    fn decision_rejected(info: VarDictionary);

    /// Die Partie hat begonnen; `state` = Spielzustand (siehe Protokoll).
    #[signal]
    fn match_started(state: VarDictionary);

    /// Antwort auf [method request_state].
    #[signal]
    fn state_received(state: VarDictionary);

    /// Antwort auf [method debug_request_card_names]: alle Kartennamen der Kartendatenbank,
    /// sortiert (Debug-Fenster).
    #[signal]
    fn debug_card_names_received(names: PackedStringArray);

    /// Die Partie ist vorbei: `{winner, draw, reason}`.
    #[signal]
    fn game_over(result: VarDictionary);

    /// Fehler der Brücke oder beim Start (`code` z. B. "deck_not_found", "spawn_failed").
    #[signal]
    fn bridge_error(code: GString, message: GString);

    /// Der Java-Prozess ist beendet (`exit_code` -1 = unbekannt / von uns beendet).
    #[signal]
    fn bridge_stopped(exit_code: i64);

    /// Startet den Java-Prozess. Gibt false zurück, wenn er schon läuft oder nicht starten
    /// konnte (dann zusätzlich [signal bridge_error]). Fertig ist er erst mit
    /// [signal bridge_ready] (Forge braucht einige Sekunden zum Laden).
    #[func]
    pub fn start_bridge(&mut self) -> bool {
        if self.child.is_some() || self.socket.is_some() {
            return false;
        }
        // Gestartet von einem Gastgeber (NeoForge) mit `-- --clash-port=N`: dorthin verbinden
        if let Some(port) = host_port() {
            return self.connect_host(port);
        }
        let forge_dir = self.resolved_forge_dir();
        let forge_jar = match self.resolved_forge_jar(&forge_dir) {
            Some(j) => j,
            None => {
                self.emit_error("forge_jar_not_found", &format!("Kein Forge-Jar in {}", forge_dir.display()));
                return false;
            }
        };
        let bridge_jar = PathBuf::from(globalize(&self.bridge_jar));
        if !bridge_jar.is_file() {
            self.emit_error(
                "bridge_jar_not_found",
                &format!("{} fehlt – forge-bridge/build.sh ausführen", bridge_jar.display()),
            );
            return false;
        }
        let log_path = globalize(&GString::from("user://forge-bridge.log"));
        let stderr = File::create(&log_path).map(Stdio::from).unwrap_or_else(|_| Stdio::null());

        let sep = if cfg!(windows) { ";" } else { ":" };
        let classpath = format!("{}{}{}", bridge_jar.display(), sep, forge_jar.display());
        let spawned = Command::new(self.java_path.to_string())
            .arg(format!("-Xmx{}m", self.java_max_memory_mb))
            .arg("-Dfile.encoding=UTF-8")
            .arg("-Djava.awt.headless=true")
            .arg("-cp")
            .arg(classpath)
            .arg("clash.forgebridge.BridgeMain")
            .current_dir(&forge_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                self.emit_error("spawn_failed", &format!("{}: {e}", self.java_path));
                return false;
            }
        };

        let stdout = child.stdout.take().expect("stdout ist gepiped");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(Incoming::Line(l)).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(Incoming::Closed);
        });

        self.stdin = child.stdin.take();
        self.child = Some(child);
        self.rx = Some(rx);
        self.ready = false;
        true
    }

    /// Verbindet sich mit dem Gastgeber auf `127.0.0.1:port` (eingebetteter Betrieb). Der
    /// Gastgeber startet die Partie selbst; Nachrichten und Befehle wie beim Java-Prozess.
    fn connect_host(&mut self, port: u16) -> bool {
        let stream = match TcpStream::connect(("127.0.0.1", port)) {
            Ok(s) => s,
            Err(e) => {
                self.emit_error("connect_failed", &format!("Gastgeber auf Port {port}: {e}"));
                return false;
            }
        };
        let reader = match stream.try_clone() {
            Ok(r) => r,
            Err(e) => {
                self.emit_error("connect_failed", &e.to_string());
                return false;
            }
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(Incoming::Line(l)).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(Incoming::Closed);
        });
        self.socket = Some(stream);
        self.rx = Some(rx);
        self.ready = false;
        true
    }

    /// Läuft die Brücke eingebettet in einem Gastgeber (NeoForge startet die Partie)?
    #[func]
    pub fn is_hosted(&self) -> bool {
        self.socket.is_some() || host_port().is_some()
    }

    /// Beendet die Brücke (Befehl "quit", nach 2 s hart beendet). Sendet
    /// [signal bridge_stopped] mit -1, falls sie lief.
    #[func]
    fn stop_bridge(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            let _ = socket.write_all(b"{\"cmd\":\"quit\"}\n");
            let _ = socket.shutdown(std::net::Shutdown::Both);
            self.rx = None;
            self.ready = false;
            self.signals().bridge_stopped().emit(-1);
            return;
        }
        let Some(mut child) = self.child.take() else {
            return;
        };
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"{\"cmd\":\"quit\"}\n");
            let _ = stdin.flush();
        }
        self.rx = None;
        self.ready = false;
        thread::spawn(move || {
            for _ in 0..20 {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
        });
        self.signals().bridge_stopped().emit(-1);
    }

    /// Läuft der Java-Prozess?
    #[func]
    pub fn is_running(&self) -> bool {
        self.child.is_some() || self.socket.is_some()
    }

    /// Hat Forge fertig geladen ([signal bridge_ready] empfangen)?
    #[func]
    fn is_ready(&self) -> bool {
        self.ready
    }

    /// Schickt einen beliebigen Befehl (Dictionary mit "cmd", siehe Protokoll).
    /// false, wenn die Brücke nicht läuft.
    #[func]
    fn send_command(&mut self, command: VarDictionary) -> bool {
        let line = Json::stringify(&command.to_variant());
        let ok = if let Some(socket) = self.socket.as_mut() {
            writeln!(socket, "{line}").and_then(|_| socket.flush()).is_ok()
        } else if let Some(stdin) = self.stdin.as_mut() {
            writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_ok()
        } else {
            return false;
        };
        if !ok {
            self.emit_error("write_failed", "Befehl konnte nicht gesendet werden");
        }
        ok
    }

    /// Verbindungstest; Antwort kommt als Nachricht `{type: "pong", nonce}`.
    #[func]
    fn ping(&mut self, nonce: i64) -> bool {
        let mut d = VarDictionary::new();
        d.set("cmd", "ping");
        d.set("nonce", nonce);
        self.send_command(d)
    }

    /// Startet eine Partie. `decks`: Pfade zu Forge-`.dck`-Dateien (`res://` erlaubt) oder
    /// Namen von Decks aus Forges Constructed-Sammlung, einer je Sitzplatz (mind. 2).
    /// `remote_seats`: Sitzplätze, die MTG Clash steuert (Rest = Forge-KI).
    /// `names`: Spielernamen (darf kürzer sein). `seed` < 0 = zufällig.
    #[func]
    pub fn start_match(
        &mut self,
        decks: PackedStringArray,
        remote_seats: PackedInt32Array,
        names: PackedStringArray,
        seed: i64,
    ) -> bool {
        let mut deck_arr = VarArray::new();
        let mut remote_arr = VarArray::new();
        for (i, d) in decks.as_slice().iter().enumerate() {
            let path = if d.to_string().starts_with("res://") || d.to_string().starts_with("user://") {
                GString::from(globalize(d).as_str())
            } else {
                d.clone()
            };
            deck_arr.push(&path.to_variant());
            remote_arr.push(&remote_seats.as_slice().contains(&(i as i32)).to_variant());
        }
        let mut name_arr = VarArray::new();
        for n in names.as_slice() {
            name_arr.push(&n.to_variant());
        }
        let mut d = VarDictionary::new();
        d.set("cmd", "start_match");
        d.set("decks", &deck_arr);
        d.set("remote", &remote_arr);
        d.set("names", &name_arr);
        if seed >= 0 {
            d.set("seed", seed);
        }
        self.send_command(d)
    }

    /// Beantwortet eine Anfrage aus [signal decision_requested]. `choice` ist eine Antwort im
    /// Format des Protokoll-Modells (Protokoll v3), z. B. `{"type": "Action", "action": {"type": "Pass"}}`;
    /// Aufbau siehe `docs/Forge-Schnittstelle.md`.
    #[func]
    pub fn answer(&mut self, request_id: i64, choice: Variant) -> bool {
        let mut d = VarDictionary::new();
        d.set("cmd", "answer");
        d.set("id", request_id);
        d.set("choice", &normalize_numbers(choice));
        self.send_command(d)
    }

    /// Fragt den aktuellen Spielzustand ab → [signal state_received]. `viewer` = Sitzplatz,
    /// dessen Handkarten mitgeschickt werden (-1 = keiner).
    #[func]
    pub fn request_state(&mut self, viewer: i64) -> bool {
        let mut d = VarDictionary::new();
        d.set("cmd", "state");
        d.set("viewer", viewer);
        self.send_command(d)
    }

    /// Debug: alle Kartennamen anfordern → [signal debug_card_names_received].
    #[func]
    pub fn debug_request_card_names(&mut self) -> bool {
        let mut d = VarDictionary::new();
        d.set("cmd", "debug_card_names");
        self.send_command(d)
    }

    /// Debug: Karte `name` neu erzeugen und Sitzplatz `seat` auf die Hand legen. Danach kommt
    /// der neue Zustand; eine offene Priorität wird neu gestellt.
    #[func]
    pub fn debug_add_card(&mut self, seat: i64, name: GString) -> bool {
        let mut d = VarDictionary::new();
        d.set("cmd", "debug_add_card");
        d.set("seat", seat);
        d.set("name", &name);
        self.send_command(d)
    }

    /// Debug: unendliches Mana für Sitzplatz `seat` an/aus (Pool wird bei jedem Phasenwechsel
    /// und Zauber auf 50 je Manaart aufgefüllt).
    #[func]
    pub fn debug_infinite_mana(&mut self, seat: i64, on: bool) -> bool {
        let mut d = VarDictionary::new();
        d.set("cmd", "debug_infinite_mana");
        d.set("seat", seat);
        d.set("on", on);
        self.send_command(d)
    }
}

impl ForgeBridge {
    fn resolved_forge_dir(&self) -> PathBuf {
        if !self.forge_dir.is_empty() {
            return PathBuf::from(globalize(&self.forge_dir));
        }
        if let Ok(d) = std::env::var("FORGE_DIR") {
            return PathBuf::from(d);
        }
        let local = PathBuf::from(globalize(&GString::from("res://../forge-bridge/forge")));
        if local.join("res").is_dir() {
            return local;
        }
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
        Path::new(&home).join("Games").join("Forge")
    }

    fn resolved_forge_jar(&self, forge_dir: &Path) -> Option<PathBuf> {
        if !self.forge_jar.is_empty() {
            let p = PathBuf::from(globalize(&self.forge_jar));
            return p.is_file().then_some(p);
        }
        let mut jars: Vec<PathBuf> = std::fs::read_dir(forge_dir)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n.starts_with("forge-gui-desktop-") && n.ends_with("-jar-with-dependencies.jar")
                })
            })
            .collect();
        jars.sort();
        jars.pop()
    }

    fn emit_error(&mut self, code: &str, message: &str) {
        godot_warn!("ForgeBridge: {code}: {message}");
        self.signals().bridge_error().emit(&GString::from(code), &GString::from(message));
    }

    fn dispatch(&mut self, line: &str) {
        let parsed = Json::parse_string(line);
        let Ok(dict) = normalize_numbers(parsed).try_to::<VarDictionary>() else {
            self.emit_error("bad_message", line);
            return;
        };
        if self.log_messages {
            godot_print!("ForgeBridge <- {line}");
        }
        self.signals().message_received().emit(&dict);
        let get_dict = |key: &str| {
            dict.get(key).and_then(|v| v.try_to::<VarDictionary>().ok()).unwrap_or_default()
        };
        let kind = dict.get("type").map(|v| v.to_string()).unwrap_or_default();
        match kind.as_str() {
            "ready" => {
                self.ready = true;
                let protocol = dict.get("protocol").and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0);
                self.signals().bridge_ready().emit(protocol);
            }
            "event" => self.signals().forge_event().emit(&dict),
            "log" => self.signals().log_entry().emit(&dict),
            "request" => self.signals().decision_requested().emit(&dict),
            "rejected" => self.signals().decision_rejected().emit(&dict),
            "match_started" => self.signals().match_started().emit(&get_dict("state")),
            "state" => self.signals().state_received().emit(&get_dict("state")),
            "game_over" => self.signals().game_over().emit(&dict),
            "debug_card_names" => {
                let names: PackedStringArray = dict
                    .get("names")
                    .and_then(|v| v.try_to::<VarArray>().ok())
                    .map(|a| a.iter_shared().map(|v| GString::from(v.to_string().as_str())).collect())
                    .unwrap_or_default();
                self.signals().debug_card_names_received().emit(&names);
            }
            "error" => {
                let s = |k: &str| dict.get(k).map(|v| v.to_string()).unwrap_or_default();
                let (code, msg) = (s("code"), s("message"));
                self.emit_error(&code, &msg);
            }
            _ => {}
        }
    }

    fn on_closed(&mut self) {
        self.rx = None;
        self.stdin = None;
        self.ready = false;
        if self.socket.take().is_some() {
            // Eingebettet: der Gastgeber hat die Partie beendet → Spieltisch nach kurzer Zeit
            // schließen (die Spielende-Meldung bleibt so lange sichtbar)
            self.signals().bridge_stopped().emit(0);
            let mut tree = self.base().get_tree();
            let quit = Callable::from_object_method(&tree, "quit");
            tree.create_timer(HOSTED_CLOSE_DELAY).connect("timeout", &quit);
            return;
        }
        let code = self
            .child
            .take()
            .and_then(|mut c| c.wait().ok())
            .and_then(|s| s.code())
            .map_or(-1, i64::from);
        self.signals().bridge_stopped().emit(code);
    }
}

/// `res://`/`user://` → absoluter Pfad (andere Pfade unverändert).
fn globalize(path: &GString) -> String {
    ProjectSettings::singleton().globalize_path(path).to_string()
}

/// Wandelt ganzzahlige floats (Godot-JSON kennt nur float) rekursiv in int um.
fn normalize_numbers(v: Variant) -> Variant {
    match v.get_type() {
        VariantType::FLOAT => {
            let f = v.to::<f64>();
            if f.fract() == 0.0 && f.abs() < 9.0e15 {
                (f as i64).to_variant()
            } else {
                v
            }
        }
        VariantType::DICTIONARY => {
            let src = v.to::<VarDictionary>();
            let mut out = VarDictionary::new();
            for (k, val) in src.iter_shared() {
                out.set(&k, &normalize_numbers(val));
            }
            out.to_variant()
        }
        VariantType::ARRAY => {
            let src = v.to::<VarArray>();
            let mut out = VarArray::new();
            for val in src.iter_shared() {
                out.push(&normalize_numbers(val));
            }
            out.to_variant()
        }
        _ => v,
    }
}

/// Die Java-Brücke als Regel-Engine der UI (Protokoll v3).
impl RulesEngine for Gd<ForgeBridge> {
    fn submit(&mut self, request_id: i64, choice: &Choice) -> bool {
        self.bind_mut().answer(request_id, choice.to_var())
    }

    fn request_state(&mut self, viewer: PlayerId) -> bool {
        self.bind_mut().request_state(viewer as i64)
    }
}

/// Port des Gastgebers aus den Programmargumenten nach `--` (`--clash-port=N`), falls Godot von
/// NeoForge als Spieltisch gestartet wurde.
fn host_port() -> Option<u16> {
    godot::classes::Os::singleton()
        .get_cmdline_user_args()
        .as_slice()
        .iter()
        .find_map(|a| a.to_string().strip_prefix("--clash-port=").and_then(|p| p.parse().ok()))
}
