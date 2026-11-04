package clash.forgebridge;

import java.util.List;
import java.util.Observable;
import java.util.Observer;

import com.google.gson.JsonObject;

import forge.game.Game;
import forge.game.GameLogEntry;

/**
 * Leitet jeden neuen Eintrag von Forges Spielprotokoll ({@code GameLog}, das auch Forges
 * Oberfläche zeigt) als {"type":"log"} an den Client weiter: {@code kind} = Art des Eintrags
 * (Forges {@code GameLogEntryType}, z. B. ZONE_CHANGE, DAMAGE, STACK_ADD), {@code text} = lesbarer
 * Text, {@code card} = Karten-ID der Quelle (falls bekannt), dazu Runde.
 */
@SuppressWarnings("deprecation") // java.util.Observable ist Forges Schnittstelle des GameLog
final class LogForwarder implements Observer {
    private final Game game;
    /** Anzahl bereits gesendeter Einträge. */
    private int sent;

    LogForwarder(Game game) {
        this.game = game;
    }

    @Override
    public synchronized void update(Observable o, Object arg) {
        // getAllEntries: chronologisch (ältester zuerst)
        List<GameLogEntry> all = game.getGameLog().getAllEntries();
        for (int i = sent; i < all.size(); i++) {
            GameLogEntry e = all.get(i);
            JsonObject m = ForgeBridge.message("log");
            m.addProperty("kind", e.type().name());
            m.addProperty("text", e.message());
            if (e.sourceCard() != null) {
                m.addProperty("card", e.sourceCard().getId());
            }
            m.addProperty("turn", game.getPhaseHandler().getTurn());
            ForgeBridge.send(m);
        }
        sent = Math.max(sent, all.size());
    }
}
