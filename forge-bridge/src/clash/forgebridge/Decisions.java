package clash.forgebridge;

import java.util.ArrayList;
import java.util.List;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import forge.game.Game;
import forge.game.GameEntity;
import forge.game.card.Card;
import forge.game.player.Player;
import forge.game.spellability.SpellAbility;

/**
 * Übersetzer zwischen Forge und dem Entscheidungsmodell von NeoForge (Protokoll v3).
 *
 * <p>Jede Methode baut eine NeoForge-{@code Decision} (JSON-Objekt mit {@code "type"} = Variante,
 * Felder wie in NeoForges {@code forge-engine/src/decision.rs}), schickt sie in einer Hülle als
 * {@code request} an den Client und liest die {@code Choice}-Antwort. Rückgabe {@code null} =
 * abgebrochen ({@code Choice::Cancel}) oder ungültig; die aufrufende Methode fällt dann auf die
 * Forge-KI zurück bzw. bricht das Wirken ab.
 *
 * <p>Varianten, die es in NeoForge noch nicht gibt ({@code ChooseOption}, {@code ChooseOptions},
 * {@code ChooseNumber}, {@code Choice::Cancel}), sind in {@code docs/Forge-Schnittstelle.md} unter
 * „Erweiterungen gegenüber NeoForge“ aufgeführt.
 *
 * <p>Hülle: {@code {"type":"request","id","player","decision","prompt","source":{id,name},
 * "cancelable","labels","state"}} – {@code labels} = Anzeigetext je Eintrag der Hauptliste der
 * Entscheidung (actions, candidates, options …).
 */
final class Decisions {
    private Decisions() {}

    /** Eine Option der Erweiterungs-Auswahlen: Text und optional Farbe (W U B R G, C = farblos). */
    record Opt(String text, String color) {
        static Opt of(Object o) {
            return new Opt(String.valueOf(o), null);
        }
    }

    static List<Opt> opts(List<?> items) {
        List<Opt> list = new ArrayList<>();
        for (Object o : items) {
            list.add(Opt.of(o));
        }
        return list;
    }

    // ------------------------------------------------------------------ Versand

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

    static JsonObject decision(String type, int seat) {
        JsonObject d = new JsonObject();
        d.addProperty("type", type);
        d.addProperty("player", seat);
        return d;
    }

    /**
     * Schickt {@code decision} mit Hülle und wartet auf die Antwort. Gibt das {@code Choice}-Objekt
     * zurück; {@code null} bei {@code Cancel}, fehlender oder unlesbarer Antwort.
     */
    static JsonObject ask(Game game, int seat, JsonObject decision, String prompt, SpellAbility sa, Card card,
            boolean cancelable, List<String> labels) {
        Card src = source(sa, card);
        JsonObject env = new JsonObject();
        env.add("decision", decision);
        env.addProperty("prompt", withName(prompt == null ? "" : prompt, src));
        if (src != null) {
            JsonObject s = new JsonObject();
            s.addProperty("id", src.getId());
            s.addProperty("name", src.getName());
            env.add("source", s);
        }
        if (sa != null) {
            env.addProperty("ability", withName(sa.toUnsuppressedString(), src));
        }
        env.addProperty("cancelable", cancelable);
        if (labels != null) {
            JsonArray arr = new JsonArray();
            labels.forEach(l -> arr.add(withName(l, src)));
            env.add("labels", arr);
        }
        env.add("state", StateJson.snapshot(game, seat));
        JsonElement a = ForgeBridge.request(seat, env);
        if (a == null || !a.isJsonObject()) {
            return null;
        }
        JsonObject choice = a.getAsJsonObject();
        return "Cancel".equals(type(choice)) ? null : choice;
    }

    static String type(JsonObject o) {
        return o != null && o.has("type") ? o.get("type").getAsString() : "";
    }

    private static Integer number(JsonObject choice) {
        if (!"Number".equals(type(choice)) || !choice.has("value")) {
            return null;
        }
        return choice.get("value").getAsInt();
    }

    private static List<Integer> intList(JsonObject choice, String type, String field) {
        if (!type.equals(type(choice)) || !choice.has(field) || !choice.get(field).isJsonArray()) {
            return null;
        }
        List<Integer> list = new ArrayList<>();
        for (JsonElement e : choice.getAsJsonArray(field)) {
            list.add(e.getAsInt());
        }
        return list;
    }

    private static JsonArray optionsJson(List<Opt> options, Card src) {
        JsonArray arr = new JsonArray();
        for (Opt o : options) {
            JsonObject j = new JsonObject();
            j.addProperty("text", withName(o.text(), src));
            if (o.color() != null) {
                j.addProperty("color", o.color());
            }
            arr.add(j);
        }
        return arr;
    }

    private static boolean valid(List<Integer> chosen, int size, int min, int max, boolean repeat) {
        if (chosen == null || chosen.size() < min || chosen.size() > max) {
            return false;
        }
        for (int i = 0; i < chosen.size(); i++) {
            int v = chosen.get(i);
            if (v < 0 || v >= size || (!repeat && chosen.indexOf(v) != i)) {
                return false;
            }
        }
        return true;
    }

    // ------------------------------------------------------------------ NeoForge-Entscheidungen

    /** {@code Mulligan}: true = behalten ({@code Keep}), false = Mulligan, null = unlesbar. */
    static Boolean mulligan(Game game, int seat, int mulligansTaken) {
        JsonObject d = decision("Mulligan", seat);
        d.addProperty("mulligans_taken", mulligansTaken);
        JsonObject c = ask(game, seat, d, "Starthand behalten?", null, null, false, null);
        return switch (type(c)) {
            case "Keep" -> true;
            case "Mulligan" -> false;
            default -> null;
        };
    }

    /** {@code ChooseModes}: Indizes der gewählten Modi (Wiederholung nur bei {@code canRepeat}). */
    static List<Integer> modes(Game game, int seat, SpellAbility sa, List<String> texts, int min, int max,
            boolean canRepeat, boolean cancelable) {
        JsonObject d = decision("ChooseModes", seat);
        JsonArray modes = new JsonArray();
        for (int i = 0; i < texts.size(); i++) {
            JsonArray m = new JsonArray();
            m.add(i);
            m.add(withName(texts.get(i), source(sa, null)));
            modes.add(m);
        }
        d.add("modes", modes);
        d.addProperty("min", min);
        d.addProperty("max", max);
        d.addProperty("can_repeat", canRepeat);
        String prompt = min == max ? (max == 1 ? "Wähle einen Modus" : "Wähle " + max + " Modi")
                : "Wähle " + min + " bis " + max + " Modi";
        List<Integer> chosen = intList(ask(game, seat, d, prompt, sa, null, cancelable, texts), "Modes", "modes");
        return valid(chosen, texts.size(), min, max, canRepeat) ? chosen : null;
    }

    /**
     * {@code ChooseTargets}: Indizes der gewählten Kandidaten. Kandidaten sind Karten
     * ({@code {"card": id}}) oder Spieler ({@code {"player": sitz}}).
     */
    static List<Integer> targets(Game game, int seat, SpellAbility sa, String description,
            List<GameEntity> candidates, List<String> labels, int min, int max) {
        JsonObject d = decision("ChooseTargets", seat);
        Card src = sa.getHostCard();
        d.addProperty("source", src == null ? -1 : src.getId());
        d.addProperty("description", withName(description, src));
        JsonArray arr = new JsonArray();
        for (GameEntity e : candidates) {
            arr.add(targetJson(game, e));
        }
        d.add("candidates", arr);
        d.addProperty("min", min);
        d.addProperty("max", max);
        JsonObject c = ask(game, seat, d, description, sa, null, true, labels);
        if (!"Targets".equals(type(c)) || !c.has("targets")) {
            return null;
        }
        List<Integer> chosen = new ArrayList<>();
        for (JsonElement t : c.getAsJsonArray("targets")) {
            int idx = indexOfTarget(arr, t);
            if (idx < 0) {
                return null;
            }
            chosen.add(idx);
        }
        return valid(chosen, candidates.size(), min, max, false) ? chosen : null;
    }

    static JsonObject targetJson(Game game, GameEntity e) {
        JsonObject t = new JsonObject();
        if (e instanceof Card c) {
            t.addProperty("card", c.getId());
        } else if (e instanceof Player p) {
            t.addProperty("player", game.getPlayers().indexOf(p));
        }
        return t;
    }

    private static int indexOfTarget(JsonArray candidates, JsonElement t) {
        for (int i = 0; i < candidates.size(); i++) {
            if (candidates.get(i).equals(t)) {
                return i;
            }
        }
        return -1;
    }

    /** {@code ChooseX} (X ab 0) bzw. {@code ChooseNumber} (sonst). Abbrechen erlaubt. */
    static Integer chooseX(Game game, int seat, SpellAbility sa, String varName, int min, int max) {
        if (min != 0) {
            return number(game, seat, "Wert für " + varName + " wählen", sa, null, min, max, true);
        }
        JsonObject d = decision("ChooseX", seat);
        d.addProperty("max", Math.max(0, max));
        Integer v = number(ask(game, seat, d, "Wert für " + varName + " wählen", sa, null, true, null));
        return v != null && v >= 0 && v <= max ? v : null;
    }

    /** {@code ChooseColor}: Index in {@code letters} (W U B R G). */
    static Integer color(Game game, int seat, String prompt, SpellAbility sa, List<String> letters) {
        JsonObject d = decision("ChooseColor", seat);
        JsonArray arr = new JsonArray();
        letters.forEach(arr::add);
        d.add("options", arr);
        JsonObject c = ask(game, seat, d, prompt, sa, null, false, null);
        if (!"Color".equals(type(c)) || !c.has("color")) {
            return null;
        }
        int i = letters.indexOf(c.get("color").getAsString());
        return i >= 0 ? i : null;
    }

    /** {@code NameCard} bzw. {@code ChooseType}: Index in {@code options}. */
    static Integer nameOrType(Game game, int seat, String type, String prompt, SpellAbility sa, Card card,
            List<String> options, boolean cancelable) {
        if (options.size() == 1 && !cancelable) {
            return 0;
        }
        JsonObject d = decision(type, seat);
        JsonArray arr = new JsonArray();
        options.forEach(arr::add);
        d.add("options", arr);
        Integer i = number(ask(game, seat, d, prompt, sa, card, cancelable, null));
        return i != null && i >= 0 && i < options.size() ? i : null;
    }

    /** {@code Confirm}: allgemeine Ja/Nein-Frage. */
    static Boolean confirm(Game game, int seat, String prompt, SpellAbility sa, Card card) {
        JsonObject d = decision("Confirm", seat);
        d.addProperty("prompt", withName(prompt, source(sa, card)));
        return yesNo(ask(game, seat, d, prompt, sa, card, false, null));
    }

    /** {@code OptionalTrigger}: „Du darfst …“-Trigger nutzen? */
    static Boolean optionalTrigger(Game game, int seat, String description, SpellAbility sa, Card card) {
        JsonObject d = decision("OptionalTrigger", seat);
        d.addProperty("description", withName(description, source(sa, card)));
        return yesNo(ask(game, seat, d, description, sa, card, false, null));
    }

    private static Boolean yesNo(JsonObject c) {
        return switch (type(c)) {
            case "Yes" -> true;
            case "No" -> false;
            default -> null;
        };
    }

    /** {@code Priority}: gibt das gewählte {@code Action}-Objekt zurück (null = unlesbar). */
    static JsonObject priority(Game game, int seat, JsonArray actions, List<String> labels) {
        JsonObject d = decision("Priority", seat);
        d.add("actions", actions);
        JsonObject c = ask(game, seat, d, "", null, null, false, labels);
        if (!"Action".equals(type(c)) || !c.has("action") || !c.get("action").isJsonObject()) {
            return null;
        }
        return c.getAsJsonObject("action");
    }

    // ------------------------------------------------------------------ Erweiterungen

    /** Erweiterung {@code ChooseOption}: Index der Option. Eine Option ohne Abbrechen: nicht fragen. */
    static Integer option(Game game, int seat, String prompt, SpellAbility sa, Card card, List<Opt> options,
            boolean cancelable) {
        if (options.isEmpty()) {
            return null;
        }
        if (options.size() == 1 && !cancelable) {
            return 0;
        }
        JsonObject d = decision("ChooseOption", seat);
        d.addProperty("prompt", withName(prompt, source(sa, card)));
        d.add("options", optionsJson(options, source(sa, card)));
        Integer i = number(ask(game, seat, d, prompt, sa, card, cancelable, null));
        return i != null && i >= 0 && i < options.size() ? i : null;
    }

    /** Erweiterung {@code ChooseOptions}: {@code min}–{@code max} Indizes (genau eine → {@code ChooseOption}). */
    static List<Integer> options(Game game, int seat, String prompt, SpellAbility sa, Card card, List<Opt> options,
            int min, int max, boolean cancelable) {
        max = Math.min(max, options.size());
        min = Math.max(0, Math.min(min, max));
        if (min == 1 && max == 1) {
            Integer i = option(game, seat, prompt, sa, card, options, cancelable);
            return i == null ? null : new ArrayList<>(List.of(i));
        }
        JsonObject d = decision("ChooseOptions", seat);
        d.addProperty("prompt", withName(prompt, source(sa, card)));
        d.add("options", optionsJson(options, source(sa, card)));
        d.addProperty("min", min);
        d.addProperty("max", max);
        List<Integer> chosen = intList(ask(game, seat, d, prompt, sa, card, cancelable, null), "Modes", "modes");
        return valid(chosen, options.size(), min, max, false) ? chosen : null;
    }

    /** Erweiterung: zwei beschriftete Knöpfe (Kopf/Zahl, Tappen/Enttappen …); true = erster. */
    static Boolean binary(Game game, int seat, String prompt, SpellAbility sa, Card card, String first,
            String second) {
        Integer i = option(game, seat, prompt, sa, card, List.of(new Opt(first, null), new Opt(second, null)), false);
        return i == null ? null : i == 0;
    }

    /** Erweiterung {@code ChooseNumber}: Zahl zwischen {@code min} und {@code max}. */
    static Integer number(Game game, int seat, String prompt, SpellAbility sa, Card card, int min, int max,
            boolean cancelable) {
        max = Math.max(min, max);
        JsonObject d = decision("ChooseNumber", seat);
        d.addProperty("prompt", withName(prompt, source(sa, card)));
        d.addProperty("min", min);
        d.addProperty("max", max);
        Integer v = number(ask(game, seat, d, prompt, sa, card, cancelable, null));
        return v != null && v >= min && v <= max ? v : null;
    }
}
