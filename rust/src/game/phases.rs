//! Phasen eines Zugs (Anzeige im Zeitstrahl des HUD). Welche Phase gerade läuft, bestimmt
//! Forge; [`crate::forge::table`] übersetzt Forges `PhaseType` in diese Phasen.



#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Untap,
    Upkeep,
    Draw,
    Main1,
    Attackers,
    Blockers,
    Damage,
    Main2,
    End,
}

impl Phase {
    /// Phasen in Anzeigereihenfolge.
    pub const ORDER: [Phase; 9] = [
        Phase::Untap,
        Phase::Upkeep,
        Phase::Draw,
        Phase::Main1,
        Phase::Attackers,
        Phase::Blockers,
        Phase::Damage,
        Phase::Main2,
        Phase::End,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Phase::Untap => tr!("Enttappen", "Untap"),
            Phase::Upkeep => tr!("Auffrischung", "Upkeep"),
            Phase::Draw => tr!("Ziehen", "Draw"),
            Phase::Main1 => tr!("Hauptphase 1", "Main phase 1"),
            Phase::Attackers => tr!("Angreifer bestimmen", "Declare attackers"),
            Phase::Blockers => tr!("Blocker zuweisen", "Declare blockers"),
            Phase::Damage => tr!("Schadensberechnung", "Combat damage"),
            Phase::Main2 => tr!("Hauptphase 2", "Main phase 2"),
            Phase::End => tr!("Ende", "End"),
        }
    }

}
