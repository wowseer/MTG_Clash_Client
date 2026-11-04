package clash.forgebridge;

import com.google.common.eventbus.Subscribe;
import com.google.gson.JsonObject;

import forge.game.Game;
import forge.game.event.GameEvent;

/**
 * Leitet jedes Forge-{@link GameEvent} als {"type":"event"} an den Client weiter:
 * "event" = Klassenname (z. B. GameEventCardChangeZone), "text" = Forges Beschreibung,
 * dazu Runde und Phase.
 */
final class EventForwarder {
    private final Game game;

    EventForwarder(Game game) {
        this.game = game;
    }

    @Subscribe
    public void onEvent(GameEvent ev) {
        JsonObject m = ForgeBridge.message("event");
        m.addProperty("event", ev.getClass().getSimpleName());
        m.addProperty("text", String.valueOf(ev));
        var ph = game.getPhaseHandler();
        m.addProperty("turn", ph.getTurn());
        m.addProperty("phase", ph.getPhase() == null ? null : ph.getPhase().name());
        ForgeBridge.send(m);
        // Eingebettet startet der Gastgeber die Partie, also meldet sie hier ihr Ende
        if (ForgeBridge.isEmbedded() && ev instanceof forge.game.event.GameEventGameOutcome) {
            ForgeBridge.sendGameOver(game);
        }
    }
}
