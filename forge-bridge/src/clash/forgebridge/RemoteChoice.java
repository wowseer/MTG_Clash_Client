package clash.forgebridge;

import java.util.ArrayList;
import java.util.List;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import forge.game.Game;
import forge.game.card.Card;
import forge.game.spellability.SpellAbility;

/**
 * Allgemeine Auswahl-Anfragen an den Client ({@code request} mit {@code kind = "choice"}).
 *
 * <p>Jede Methode baut eine Anfrage in einem von fünf Modi, wartet auf die Antwort und gibt
 * sie geprüft zurück. {@code null} bedeutet: abgebrochen oder ungültig. Die aufrufende Methode
 * fällt dann auf die Forge-KI zurück bzw. liefert das Abbruch-Ergebnis.
 *
 * <ul>
 *   <li>{@code single}: genau eine Option → Antwort Index (gibt es nur eine und ist Abbrechen
 *       nicht erlaubt, wird nicht gefragt)</li>
 *   <li>{@code multi}: {@code min}–{@code max} Optionen → Antwort Liste von Indizes (genau eine
 *       wird als {@code single} gefragt)</li>
 *   <li>{@code confirm}: zwei Knöpfe (ja/nein) → Antwort true/false</li>
 *   <li>{@code number}: Zahl zwischen {@code min} und {@code max} → Antwort Zahl</li>
 *   <li>{@code text}: freie Eingabe (z. B. Kartenname) → Antwort Text</li>
 * </ul>
 */
final class RemoteChoice {
    private RemoteChoice() {}

    /**
     * Eine Option: Anzeigetext, optional eine Farbe (W, U, B, R, G, C) für farbige Knöpfe und
     * optional das Spielobjekt, für das sie steht (Karten-ID bzw. Sitzplatz; z. B. bei Zielen),
     * damit der Client es auf dem Tisch hervorheben kann.
     */
    record Option(String text, String color, Integer cardId, Integer player) {
        Option(String text, String color) {
            this(text, color, null, null);
        }

        static Option of(Object o) {
            return new Option(String.valueOf(o), null);
        }
    }

    static List<Option> options(List<?> items) {
        List<Option> list = new ArrayList<>();
        for (Object o : items) {
            list.add(Option.of(o));
        }
        return list;
    }

    private static JsonObject base(Game game, int seat, String mode, String title, SpellAbility sa, Card card,
            boolean cancelable) {
        JsonObject p = new JsonObject();
        p.addProperty("mode", mode);
        Card source = card != null ? card : sa == null ? null : sa.getHostCard();
        p.addProperty("title", title == null ? "" : withName(title, source));
        if (source != null) {
            JsonObject s = new JsonObject();
            s.addProperty("id", source.getId());
            s.addProperty("name", source.getName());
            p.add("source", s);
        }
        if (sa != null) {
            p.addProperty("ability", withName(sa.toUnsuppressedString(), source));
        }
        p.addProperty("cancelable", cancelable);
        p.add("state", StateJson.snapshot(game, seat));
        return p;
    }

    /** Ersetzt Forges Platzhalter {@code CARDNAME} / {@code NICKNAME} durch den Kartennamen. */
    static String withName(String text, Card card) {
        if (text == null || card == null) {
            return text;
        }
        return text.replace("CARDNAME", card.getName()).replace("NICKNAME", card.getName());
    }

    private static Card source(SpellAbility sa, Card card) {
        return card != null ? card : sa == null ? null : sa.getHostCard();
    }

    private static void addOptions(JsonObject p, List<Option> options, Card source) {
        JsonArray arr = new JsonArray();
        for (Option o : options) {
            JsonObject j = new JsonObject();
            j.addProperty("text", withName(o.text(), source));
            if (o.color() != null) {
                j.addProperty("color", o.color());
            }
            if (o.cardId() != null) {
                j.addProperty("card_id", o.cardId());
            }
            if (o.player() != null) {
                j.addProperty("player", o.player());
            }
            arr.add(j);
        }
        p.add("options", arr);
    }

    /** Genau eine Option wählen. Gibt den Index zurück oder null (Abbruch/ungültig). */
    static Integer single(Game game, int seat, String title, SpellAbility sa, Card card, List<Option> options,
            boolean cancelable) {
        if (options.isEmpty()) {
            return null;
        }
        if (options.size() == 1 && !cancelable) {
            return 0; // nur eine Möglichkeit: nicht fragen
        }
        JsonObject p = base(game, seat, "single", title, sa, card, cancelable);
        addOptions(p, options, source(sa, card));
        JsonElement a = ForgeBridge.request("choice", seat, p);
        Integer i = asInt(a);
        return i != null && i >= 0 && i < options.size() ? i : null;
    }

    /** {@code min}–{@code max} verschiedene Optionen wählen. Gibt die Indizes zurück oder null. */
    static List<Integer> multi(Game game, int seat, String title, SpellAbility sa, Card card, List<Option> options,
            int min, int max, boolean cancelable) {
        max = Math.min(max, options.size());
        min = Math.max(0, Math.min(min, max));
        if (min == 1 && max == 1) {
            // Genau eine: als Einfachauswahl fragen (ein Klick statt Auswahl + „Fertig“)
            Integer i = single(game, seat, title, sa, card, options, cancelable);
            return i == null ? null : new ArrayList<>(List.of(i));
        }
        JsonObject p = base(game, seat, "multi", title, sa, card, cancelable);
        addOptions(p, options, source(sa, card));
        p.addProperty("min", min);
        p.addProperty("max", max);
        JsonElement a = ForgeBridge.request("choice", seat, p);
        if (a == null || !a.isJsonArray()) {
            return null;
        }
        List<Integer> result = new ArrayList<>();
        for (JsonElement e : a.getAsJsonArray()) {
            Integer i = asInt(e);
            if (i == null || i < 0 || i >= options.size() || result.contains(i)) {
                return null;
            }
            result.add(i);
        }
        return result.size() >= min && result.size() <= max ? result : null;
    }

    /** Ja/Nein mit eigenen Beschriftungen. Gibt true (erster Knopf) / false zurück oder null. */
    static Boolean confirm(Game game, int seat, String title, SpellAbility sa, Card card, String yes, String no) {
        JsonObject p = base(game, seat, "confirm", title, sa, card, false);
        addOptions(p, List.of(new Option(yes, null), new Option(no, null)), source(sa, card));
        JsonElement a = ForgeBridge.request("choice", seat, p);
        return a != null && a.isJsonPrimitive() && a.getAsJsonPrimitive().isBoolean() ? a.getAsBoolean() : null;
    }

    /** Zahl zwischen min und max. Gibt die Zahl zurück oder null. */
    static Integer number(Game game, int seat, String title, SpellAbility sa, Card card, int min, int max,
            boolean cancelable) {
        JsonObject p = base(game, seat, "number", title, sa, card, cancelable);
        p.addProperty("min", min);
        p.addProperty("max", Math.max(min, max));
        Integer i = asInt(ForgeBridge.request("choice", seat, p));
        return i != null && i >= min && i <= Math.max(min, max) ? i : null;
    }

    /** Freie Texteingabe; {@code error} wird angezeigt (z. B. nach einer ungültigen Eingabe). */
    static String text(Game game, int seat, String title, SpellAbility sa, Card card, String error,
            boolean cancelable) {
        JsonObject p = base(game, seat, "text", title, sa, card, cancelable);
        if (error != null) {
            p.addProperty("error", error);
        }
        JsonElement a = ForgeBridge.request("choice", seat, p);
        return a != null && a.isJsonPrimitive() && a.getAsJsonPrimitive().isString() ? a.getAsString().trim() : null;
    }

    private static Integer asInt(JsonElement a) {
        if (a == null || !a.isJsonPrimitive() || !a.getAsJsonPrimitive().isNumber()) {
            return null;
        }
        return a.getAsInt();
    }
}
