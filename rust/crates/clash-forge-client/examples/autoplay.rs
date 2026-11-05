//! Beispiel: ein Spieltisch ohne Grafik, der Forge nur über `clash-protocol` und
//! `clash-forge-client` bedient – Vorlage für eigene Oberflächen.
//!
//! Er spielt einen Sitz mit einer einfachen Regel: Mulligan behalten, ein Land spielen, wenn
//! möglich, sonst passen; nicht angreifen, nicht blocken; Mana automatisch bezahlen; bei allem
//! anderen das Erste bzw. Kleinste nehmen.
//!
//! Eigenständig (startet die Java-Brücke):
//! ```sh
//! cargo run -p clash-forge-client --example autoplay -- <forge-ordner> <forge-bridge.jar> <deck1.dck> <deck2.dck>
//! ```
//! Als Tisch eines Gastgebers (z. B. NeoForge mit `CLASH_GODOT=<dieses Programm>`): erkennt
//! `--clash-port=N` in den Argumenten und verbindet sich dorthin.

use std::path::PathBuf;
use std::time::Duration;

use clash_forge_client::{host_port, ForgeClient, Incoming, LaunchConfig};
use clash_protocol::{command, message_type, Action, Choice, Decision, PayAction, Request, RulesEngine};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut client = match host_port(&args) {
        Some(port) => ForgeClient::connect(port).expect("Gastgeber nicht erreichbar"),
        None => {
            let [forge, jar, d1, d2, ..] = args.as_slice() else {
                eprintln!("Aufruf: autoplay <forge-ordner> <forge-bridge.jar> <deck1> <deck2>");
                std::process::exit(2);
            };
            let mut cfg = LaunchConfig::new(forge, jar);
            cfg.stderr_log = Some(PathBuf::from(std::env::temp_dir()).join("clash-autoplay-forge.log"));
            let mut c = ForgeClient::spawn(&cfg).unwrap_or_else(|e| panic!("{e}"));
            wait_ready(&mut c);
            // Deck-Dateien absolut (die Brücke läuft im Forge-Ordner); Namen bleiben, wie sie sind
            let deck = |d: &String| std::fs::canonicalize(d).map_or_else(|_| d.clone(), |p| p.display().to_string());
            let start = command::StartMatch { decks: vec![deck(d1), deck(d2)], ai_pace: 0.0, seed: Some(1), ..Default::default() };
            c.send(&start.to_json()).expect("senden");
            c
        }
    };
    let mut answered = 0;
    loop {
        match client.recv_timeout(Duration::from_secs(120)) {
            Some(Incoming::Message(m)) => match message_type(&m) {
                "request" => {
                    let Some(req) = Request::from_json(&m) else { continue };
                    let choice = decide(&req);
                    answered += 1;
                    println!("[autoplay] #{} {} → {}", req.id, kind(&req.decision), choice.to_json());
                    client.submit(req.id, &choice);
                }
                "game_over" => {
                    println!("[autoplay] Spielende: {} ({answered} Entscheidungen)", m["reason"]);
                    break;
                }
                "error" => eprintln!("[autoplay] Fehler: {} {}", m["code"], m["message"]),
                _ => {}
            },
            Some(Incoming::Invalid(line)) => eprintln!("[autoplay] keine JSON-Zeile: {line}"),
            Some(Incoming::Closed) | None => {
                println!("[autoplay] Verbindung zu ({answered} Entscheidungen)");
                break;
            }
        }
    }
    client.stop();
}

fn wait_ready(client: &mut ForgeClient) {
    while let Some(m) = client.recv() {
        match m {
            Incoming::Message(m) if message_type(&m) == "ready" => return,
            Incoming::Closed => panic!("Brücke beendet sich beim Start"),
            _ => {}
        }
    }
}

fn kind(d: &Decision) -> String {
    format!("{d:?}").split([' ', '{', '(']).next().unwrap_or_default().to_string()
}

/// Die einfache Spielregel des Beispiels.
fn decide(req: &Request) -> Choice {
    use Decision::*;
    match &req.decision {
        Mulligan { .. } => Choice::Keep,
        ChooseStartingPlayer { .. } => Choice::Number(0),
        Priority { actions, .. } => {
            Choice::Action(actions.iter().copied().find(|a| matches!(a, Action::PlayLand(_))).unwrap_or(Action::Pass))
        }
        PayMana { .. } => Choice::Pay(PayAction::AutoPay),
        DeclareAttackers { .. } => Choice::Attackers(Vec::new()),
        DeclareBlockers { .. } => Choice::Blocks(Vec::new()),
        ChooseTargets { candidates, min, .. } => Choice::Targets(candidates.iter().copied().take(*min as usize).collect()),
        ChooseCards { options, min, .. } => Choice::Cards(options.iter().copied().take(*min as usize).collect()),
        MulliganBottom { hand, count, .. } => Choice::Cards(hand.iter().copied().take(*count as usize).collect()),
        ChooseModes { modes, min, .. } => Choice::Modes(modes.iter().map(|m| m.0).take((*min).max(1) as usize).collect()),
        ChooseOptions { min, .. } => Choice::Modes((0..*min as usize).collect()),
        OrderTriggers { descriptions, .. } => Choice::Order((0..descriptions.len()).collect()),
        Scry { cards, .. } => Choice::Arrange { top: cards.clone(), bottom: Vec::new() },
        ChooseColor { options, .. } => options.first().map_or(Choice::Cancel, |c| Choice::Color(*c)),
        AssignCombatDamage { amount, blockers, .. } => {
            let mut v = vec![0; blockers.len()];
            if let Some(first) = v.first_mut() {
                *first = *amount;
            }
            Choice::Damage(v)
        }
        DivideAmount { amount, targets, .. } => {
            let mut v = vec![0; targets.len()];
            if let Some(first) = v.first_mut() {
                *first = *amount;
            }
            Choice::Damage(v)
        }
        ChooseX { .. } => Choice::Number(0),
        ChooseNumber { min, .. } => Choice::Number(*min),
        ChooseOption { .. } | NameCard { .. } | ChooseType { .. } => Choice::Number(0),
        Confirm { .. } | OptionalTrigger { .. } | PayUnlessCost { .. } | PayLifeForPhyrexian { .. } => Choice::No,
    }
}
