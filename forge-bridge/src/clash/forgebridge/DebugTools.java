package clash.forgebridge;

import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.ConcurrentHashMap;

import com.google.common.eventbus.Subscribe;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import forge.card.MagicColor;
import forge.game.Game;
import forge.game.card.Card;
import forge.game.event.GameEvent;
import forge.game.event.GameEventSpellAbilityCast;
import forge.game.event.GameEventSpellResolved;
import forge.game.event.GameEventTurnPhase;
import forge.game.mana.Mana;
import forge.game.mana.ManaPool;
import forge.game.player.Player;
import forge.item.PaperCard;
import forge.model.FModel;

/**
 * Entwickler-Werkzeuge für das Debug-Fenster der UI (nur in Debug-Builds von Godot sichtbar):
 * <ul>
 * <li>{@code debug_card_names}: alle Kartennamen der Kartendatenbank (sortiert) als Nachricht
 * {@code debug_card_names} mit {@code names}.</li>
 * <li>{@code debug_add_card} ({@code seat}, {@code name}): neue Karte erzeugen und dem Spieler
 * auf die Hand legen (wie Forges „Karte auf die Hand“ im Entwicklermodus).</li>
 * <li>{@code debug_infinite_mana} ({@code seat}, {@code on}): Manapool des Spielers bei jedem
 * Phasenwechsel, nach jedem gewirkten bzw. abgehandelten Zauber und sofort auf
 * {@link #MANA_PER_TYPE} je Manaart (WUBRG und farblos) auffüllen.</li>
 * </ul>
 * Änderungen laufen im Spiel-Thread, sobald er auf eine Antwort des Spielers wartet
 * ({@link ForgeBridge#runOnGameThread}). Nach jeder Änderung schickt die Brücke den neuen Zustand; wartet der Spieler gerade auf
 * Priorität, wird sie neu gestellt (damit die neue Karte bzw. das Mana sofort nutzbar ist).
 */
final class DebugTools {
    /** Mana je Art, auf das der Pool bei unendlichem Mana aufgefüllt wird. */
    static final int MANA_PER_TYPE = 50;
    private static final byte[] TYPES = {
        MagicColor.WHITE, MagicColor.BLUE, MagicColor.BLACK, MagicColor.RED, MagicColor.GREEN, MagicColor.COLORLESS };

    /** Sitzplätze mit unendlichem Mana. */
    private static final Set<Integer> INFINITE = ConcurrentHashMap.newKeySet();
    /** Quelle des Debug-Manas je Spiel und Sitzplatz (unsichtbare Karte, nie in einer Zone). */
    private static final Map<Player, Card> SOURCES = new ConcurrentHashMap<>();

    private DebugTools() {
    }

    /** Alle Kartennamen (eindeutig, sortiert). */
    static void sendCardNames() {
        TreeSet<String> names = new TreeSet<>();
        for (PaperCard pc : FModel.getMagicDb().getCommonCards().getUniqueCards()) {
            names.add(pc.getName());
        }
        JsonArray arr = new JsonArray();
        names.forEach(arr::add);
        JsonObject m = ForgeBridge.message("debug_card_names");
        m.add("names", arr);
        ForgeBridge.send(m);
    }

    /** Karte {@code name} neu erzeugen und Sitzplatz {@code seat} auf die Hand legen. */
    static void addCardToHand(Game game, int seat, String name) {
        PaperCard pc = FModel.getMagicDb().getCommonCards().getCard(name);
        if (pc == null) {
            ForgeBridge.sendError("unknown_card", "Unbekannte Karte: " + name);
            return;
        }
        Player p = player(game, seat);
        if (p == null) {
            ForgeBridge.sendError("bad_seat", "Kein Spieler an Sitzplatz " + seat);
            return;
        }
        ForgeBridge.runOnGameThread(() -> {
            Card c = Card.fromPaperCard(pc, p);
            game.getAction().moveToHand(c, null);
            ForgeBridge.debugChanged(game);
        });
    }

    /** Unendliches Mana für Sitzplatz {@code seat} an- bzw. ausschalten (an: sofort auffüllen). */
    static void setInfiniteMana(Game game, int seat, boolean on) {
        if (on) {
            INFINITE.add(seat);
        } else {
            INFINITE.remove(seat);
        }
        if (game != null && on) {
            ForgeBridge.runOnGameThread(() -> {
                refill(game);
                ForgeBridge.debugChanged(game);
            });
        }
    }

    /** Ist für Sitzplatz {@code seat} unendliches Mana an? */
    static boolean hasInfiniteMana(int seat) {
        return INFINITE.contains(seat);
    }

    /** Pools aller Spieler mit unendlichem Mana auf {@link #MANA_PER_TYPE} je Art auffüllen. */
    static void refill(Game game) {
        for (int seat : INFINITE) {
            Player p = player(game, seat);
            if (p == null) {
                continue;
            }
            Card source = SOURCES.computeIfAbsent(p, pl -> {
                Card c = new Card(game.nextCardId(), game);
                c.setOwner(pl);
                c.setName("Debug-Mana");
                return c;
            });
            ManaPool pool = p.getManaPool();
            for (byte type : TYPES) {
                int missing = MANA_PER_TYPE - pool.getAmountOfColor(type);
                for (int i = 0; i < missing; i++) {
                    pool.addManaNoEvent(new Mana(type, source, null, p));
                }
            }
        }
    }

    private static Player player(Game game, int seat) {
        if (game == null || seat < 0 || seat >= game.getRegisteredPlayers().size()) {
            return null;
        }
        return game.getRegisteredPlayers().get(seat);
    }

    /** Füllt bei Phasenwechseln und Zaubern die Pools nach (läuft im Spiel-Thread). */
    static final class Refiller {
        private final Game game;

        Refiller(Game game) {
            this.game = game;
        }

        @Subscribe
        public void onEvent(GameEvent ev) {
            if (INFINITE.isEmpty()) {
                return;
            }
            if (ev instanceof GameEventTurnPhase || ev instanceof GameEventSpellAbilityCast || ev instanceof GameEventSpellResolved) {
                refill(game);
            }
        }
    }
}
