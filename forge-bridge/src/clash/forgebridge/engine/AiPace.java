package clash.forgebridge.engine;

import forge.game.Game;
import forge.game.event.GameEvent;
import forge.game.event.GameEventAttackersDeclared;
import forge.game.event.GameEventBlockersDeclared;
import forge.game.event.GameEventLandPlayed;
import forge.game.event.GameEventSpellAbilityCast;
import forge.game.event.GameEventTurnPhase;
import forge.game.player.Player;
import forge.game.player.PlayerView;

/**
 * Tempo der Forge-KI: Die KI entscheidet in Millisekunden, am Tisch wirkte ihr Zug dadurch wie
 * ein Sprung. Nach jeder sichtbaren Aktion eines KI-Spielers wartet der Spiel-Thread kurz (der
 * Ereignis-Bus von Forge läuft synchron im Spiel-Thread), damit man ihr wie einem Menschen
 * zusehen kann und die Animationen der UI Zeit haben.
 *
 * <p>Pausen (bei Faktor 1): Land spielen 0,7 s, Zauber/Fähigkeit 1 s, Angreifer 0,9 s, Blocker
 * 0,7 s, jeder Phasenwechsel im Zug der KI 0,25 s. Der Faktor kommt aus {@code start_match}
 * ({@code ai_pace}, 0 = keine Pausen, z. B. für Tests). Ohne ferngesteuerten Spieler (KI gegen
 * KI) wird nie gewartet.
 */
final class AiPace {
    private AiPace() {}

    private static volatile double factor = 1.0;

    static void setFactor(double f) {
        factor = Math.max(0.0, f);
    }

    /** Nach dem Weiterleiten eines Ereignisses: warten, wenn es eine Aktion der KI war. */
    static void after(Game game, GameEvent ev) {
        if (factor <= 0.0 || !anyRemote(game)) {
            return;
        }
        long ms = 0;
        if (ev instanceof GameEventLandPlayed e && isAi(game, e.player())) {
            ms = 700;
        } else if (ev instanceof GameEventSpellAbilityCast e && e.si() != null
                && isAi(game, e.si().getActivatingPlayer())) {
            ms = 1000;
        } else if (ev instanceof GameEventAttackersDeclared e && isAi(game, e.player())
                && e.attackersMap() != null && !e.attackersMap().isEmpty()) {
            ms = 900;
        } else if (ev instanceof GameEventBlockersDeclared e && isAi(game, e.defendingPlayer())
                && e.blockers() != null && e.blockers().values().stream().anyMatch(m -> !m.isEmpty())) {
            ms = 700;
        } else if (ev instanceof GameEventTurnPhase e && isAi(game, e.playerTurn())) {
            ms = 250;
        }
        if (ms > 0) {
            try {
                Thread.sleep((long) (ms * factor));
            } catch (InterruptedException ie) {
                Thread.currentThread().interrupt();
            }
        }
    }

    private static boolean isAi(Game game, PlayerView view) {
        if (view == null) {
            return false;
        }
        Player p = game.getPlayer(view);
        return p != null && !(p.getController() instanceof RemotePlayerController);
    }

    private static boolean anyRemote(Game game) {
        for (Player p : game.getPlayers()) {
            if (p.getController() instanceof RemotePlayerController) {
                return true;
            }
        }
        return false;
    }
}
