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
 * Übersetzer zwischen Forge und dem Protokoll-Modell (Protokoll v3).
 *
 * <p>Jede Methode baut eine {@code Decision} (JSON-Objekt mit {@code "type"} = Variante,
 * Felder wie im Vorbild {@code forge-engine/src/decision.rs} aus forge-rules-rs), schickt sie in einer Hülle als
 * {@code request} an den Client und liest die {@code Choice}-Antwort. Rückgabe {@code null} =
 * abgebrochen ({@code Choice::Cancel}) oder ungültig; die aufrufende Methode fällt dann auf die
 * Forge-KI zurück bzw. bricht das Wirken ab.
 *
 * <p>Varianten, die es im Basismodell noch nicht gibt ({@code ChooseOption}, {@code ChooseOptions},
 * {@code ChooseNumber}, {@code Choice::Cancel}), sind in {@code docs/Forge-Schnittstelle.md} unter
 * „Erweiterungen gegenüber dem Basismodell“ aufgeführt.
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
        JsonElement a = askRaw(game, seat, decision, prompt, sa, card, cancelable, labels, null);
        if (a == null || !a.isJsonObject()) {
            return null;
        }
        JsonObject choice = a.getAsJsonObject();
        return "Cancel".equals(type(choice)) ? null : choice;
    }

    /**
     * Wie {@link #ask}, liefert aber die rohe Antwort (auch {@code Cancel}). {@code cardInfo} = Karten,
     * deren Daten mitgeschickt werden (Hülle {@code cards}), damit der Client sie auch aus
     * verdeckten Zonen (Bibliothek) darstellen kann.
     */
    static JsonElement askRaw(Game game, int seat, JsonObject decision, String prompt, SpellAbility sa, Card card,
            boolean cancelable, List<String> labels, List<Card> cardInfo) {
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
        if (cardInfo != null) {
            JsonArray cards = new JsonArray();
            cardInfo.forEach(c -> cards.add(StateJson.card(c)));
            env.add("cards", cards);
        }
        env.add("state", StateJson.snapshot(game, seat));
        return ForgeBridge.request(seat, env);
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

    // ------------------------------------------------------------------ Entscheidungen des Protokoll-Modells

    /** {@code Mulligan}: true = behalten ({@code Keep}), false = Mulligan, null = unlesbar. */
    static Boolean mulligan(Game game, int seat, int mulligansTaken, int firstPlayer) {
        JsonObject d = decision("Mulligan", seat);
        d.addProperty("mulligans_taken", mulligansTaken);
        // Erweiterung: wer die Partie beginnt (CR 103.1), für die Anzeige
        d.addProperty("first_player", firstPlayer);
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
            List<? extends forge.game.GameObject> candidates, List<String> labels, int min, int max) {
        JsonObject d = decision("ChooseTargets", seat);
        Card src = sa == null ? null : sa.getHostCard();
        d.addProperty("source", src == null ? -1 : src.getId());
        d.addProperty("description", withName(description, src));
        JsonArray arr = new JsonArray();
        for (forge.game.GameObject e : candidates) {
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

    /**
     * Ziel als {@code Target}: Karte {@code {"card": id}}, Spieler {@code {"player": sitz}};
     * ein Zauber bzw. eine Fähigkeit auf dem Stack als Karte ihrer Quelle (das Basismodell ({@code Target})
     * kennt keine Stack-Einträge).
     */
    static JsonObject targetJson(Game game, forge.game.GameObject e) {
        JsonObject t = new JsonObject();
        if (e instanceof SpellAbility s && s.getHostCard() != null) {
            t.addProperty("card", s.getHostCard().getId());
        } else if (e instanceof Card c) {
            t.addProperty("card", c.getId());
        } else if (e instanceof Player p) {
            t.addProperty("player", StateJson.indexOf(game, p));
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
        if ("Refresh".equals(type(c))) {
            return c; // Debug-Werkzeug: Priorität neu stellen
        }
        if (!"Action".equals(type(c)) || !c.has("action") || !c.get("action").isJsonObject()) {
            return null;
        }
        return c.getAsJsonObject("action");
    }

    /**
     * {@code DeclareAttackers}: gibt die Liste (Angreifer, Ziel-Index in {@code defenders}) zurück;
     * leere Liste = kein Angriff, null = unlesbar. {@code error} wird als Fragetext angezeigt
     * (nach einer ungültigen Erklärung).
     */
    static List<int[]> attackers(Game game, int seat, List<Card> candidates, List<GameEntity> defenders,
            List<String> labels, String error) {
        JsonObject d = decision("DeclareAttackers", seat);
        JsonArray cands = new JsonArray();
        candidates.forEach(c -> cands.add(c.getId()));
        d.add("candidates", cands);
        JsonArray defs = new JsonArray();
        defenders.forEach(e -> defs.add(targetJson(game, e)));
        d.add("defenders", defs);
        JsonObject c = ask(game, seat, d, error == null ? "Angreifer wählen" : error, null, null, false, labels);
        if (!"Attackers".equals(type(c)) || !c.has("attackers")) {
            return null;
        }
        List<int[]> result = new ArrayList<>();
        for (JsonElement e : c.getAsJsonArray("attackers")) {
            JsonArray pair = e.getAsJsonArray();
            int card = indexOfId(candidates, pair.get(0).getAsInt());
            int def = indexOfTarget(defs, pair.get(1));
            if (card < 0 || def < 0) {
                return null;
            }
            result.add(new int[] {card, def});
        }
        return result;
    }

    /**
     * {@code DeclareBlockers}: gibt die Liste (Blocker-Index, Angreifer-Index) zurück; leere Liste =
     * nicht blocken, null = unlesbar.
     */
    static List<int[]> blocks(Game game, int seat, List<Card> candidates, List<Card> attackers, List<String> labels,
            String error) {
        JsonObject d = decision("DeclareBlockers", seat);
        JsonArray cands = new JsonArray();
        candidates.forEach(c -> cands.add(c.getId()));
        d.add("candidates", cands);
        JsonArray atts = new JsonArray();
        attackers.forEach(c -> atts.add(c.getId()));
        d.add("attackers", atts);
        JsonObject c = ask(game, seat, d, error == null ? "Blocker wählen" : error, null, null, false, labels);
        if (!"Blocks".equals(type(c)) || !c.has("blocks")) {
            return null;
        }
        List<int[]> result = new ArrayList<>();
        for (JsonElement e : c.getAsJsonArray("blocks")) {
            JsonArray pair = e.getAsJsonArray();
            int blocker = indexOfId(candidates, pair.get(0).getAsInt());
            int attacker = indexOfId(attackers, pair.get(1).getAsInt());
            if (blocker < 0 || attacker < 0) {
                return null;
            }
            result.add(new int[] {blocker, attacker});
        }
        return result;
    }

    private static int indexOfId(List<Card> cards, int id) {
        for (int i = 0; i < cards.size(); i++) {
            if (cards.get(i).getId() == id) {
                return i;
            }
        }
        return -1;
    }

    /**
     * {@code PayMana}: {@code refs} = (Karten-ID, Index der Manafähigkeit) je Quelle,
     * {@code poolColors} = Farben im Pool, die einen Teil bezahlen können ({@code UsePool}). Gibt den
     * Index der gewählten Quelle als Text zurück, "Pool:&lt;Farbe&gt;", "AutoPay", "Cancel" oder null.
     */
    static String payMana(Game game, int seat, SpellAbility sa, String remaining, List<int[]> refs,
            List<String> labels, List<Byte> poolColors) {
        JsonObject d = decision("PayMana", seat);
        d.addProperty("remaining", remaining);
        JsonArray opts = new JsonArray();
        for (int[] r : refs) {
            JsonObject o = new JsonObject();
            o.addProperty("type", "ActivateMana");
            o.addProperty("card", r[0]);
            o.addProperty("index", r[1]);
            opts.add(o);
        }
        for (byte color : poolColors) {
            JsonObject o = new JsonObject();
            o.addProperty("type", "UsePool");
            o.addProperty("color", forge.card.MagicColor.toShortString(color));
            opts.add(o);
        }
        JsonObject auto = new JsonObject();
        auto.addProperty("type", "AutoPay");
        opts.add(auto);
        JsonObject cancel = new JsonObject();
        cancel.addProperty("type", "Cancel");
        opts.add(cancel);
        d.add("options", opts);
        List<String> all = new ArrayList<>(labels);
        poolColors.forEach(c -> all.add("Mana aus dem Pool: " + forge.card.MagicColor.toShortString(c)));
        all.add("Automatisch bezahlen");
        all.add("Abbrechen");
        JsonElement raw = askRaw(game, seat, d, "Mana bezahlen: " + remaining, sa, null, true, all, null);
        if (raw == null || !raw.isJsonObject()) {
            return null;
        }
        JsonObject c = raw.getAsJsonObject();
        if ("Cancel".equals(type(c))) {
            return "Cancel";
        }
        if (!"Pay".equals(type(c)) || !c.has("pay") || !c.get("pay").isJsonObject()) {
            return null;
        }
        JsonObject pay = c.getAsJsonObject("pay");
        switch (type(pay)) {
            case "AutoPay":
                return "AutoPay";
            case "Cancel":
                return "Cancel";
            case "UsePool": {
                String letter = pay.has("color") ? pay.get("color").getAsString() : "";
                for (byte pc : poolColors) {
                    if (forge.card.MagicColor.toShortString(pc).equals(letter)) {
                        return "Pool:" + pc;
                    }
                }
                return null;
            }
            default:
                int i = indexOfTarget(opts, pay);
                return i >= 0 && i < refs.size() ? String.valueOf(i) : null;
        }
    }

    /**
     * {@code ChooseCards}: {@code min}–{@code max} Karten aus {@code options} (Grund {@code reason},
     * z. B. "Discard", "Sacrifice", "Search"). Gibt die Indizes zurück oder null (abgebrochen bzw.
     * unlesbar). Bei genau {@code min == max == options.size()} ohne Abbrechen wird nicht gefragt.
     */
    static List<Integer> cards(Game game, int seat, String reason, List<Card> options, int min, int max, String prompt,
            SpellAbility sa, boolean cancelable) {
        max = Math.min(max, options.size());
        min = Math.max(0, Math.min(min, max));
        if (!cancelable && min == options.size()) {
            List<Integer> all = new ArrayList<>();
            for (int i = 0; i < options.size(); i++) {
                all.add(i);
            }
            return all; // keine Wahl
        }
        JsonObject d = decision("ChooseCards", seat);
        d.addProperty("reason", reason);
        JsonArray ids = new JsonArray();
        options.forEach(c -> ids.add(c.getId()));
        d.add("options", ids);
        d.addProperty("min", min);
        d.addProperty("max", max);
        JsonElement raw = askRaw(game, seat, d, prompt, sa, null, cancelable, null, options);
        if (raw == null || !raw.isJsonObject()) {
            return null;
        }
        JsonObject c = raw.getAsJsonObject();
        if (!"Cards".equals(type(c)) || !c.has("cards")) {
            return null;
        }
        List<Integer> chosen = new ArrayList<>();
        for (JsonElement e : c.getAsJsonArray("cards")) {
            int i = indexOfId(options, e.getAsInt());
            if (i < 0) {
                return null;
            }
            chosen.add(i);
        }
        return valid(chosen, options.size(), min, max, false) ? chosen : null;
    }

    /** {@code MulliganBottom}: genau {@code count} Karten aus der Hand unter die Bibliothek. */
    static List<Integer> mulliganBottom(Game game, int seat, List<Card> hand, int count) {
        JsonObject d = decision("MulliganBottom", seat);
        JsonArray ids = new JsonArray();
        hand.forEach(c -> ids.add(c.getId()));
        d.add("hand", ids);
        d.addProperty("count", count);
        JsonElement raw = askRaw(game, seat, d, "Karten unter die Bibliothek legen", null, null, false, null, hand);
        if (raw == null || !raw.isJsonObject() || !"Cards".equals(type(raw.getAsJsonObject()))) {
            return null;
        }
        List<Integer> chosen = new ArrayList<>();
        for (JsonElement e : raw.getAsJsonObject().getAsJsonArray("cards")) {
            int i = indexOfId(hand, e.getAsInt());
            if (i < 0) {
                return null;
            }
            chosen.add(i);
        }
        return valid(chosen, hand.size(), count, count, false) ? chosen : null;
    }

    /**
     * {@code Scry} (bzw. Surveil mit {@code graveyard = true}): gibt {oben, unten} als Index-Listen
     * zurück (je von oben nach unten), oder null.
     */
    static List<List<Integer>> scry(Game game, int seat, List<Card> cards, boolean graveyard) {
        JsonObject d = decision("Scry", seat);
        JsonArray ids = new JsonArray();
        cards.forEach(c -> ids.add(c.getId()));
        d.add("cards", ids);
        d.addProperty("graveyard", graveyard);
        String prompt = graveyard ? "Überwachen: oben lassen oder in den Friedhof" : "Hellsicht: oben oder unter die Bibliothek";
        JsonElement raw = askRaw(game, seat, d, prompt, null, null, false, null, cards);
        if (raw == null || !raw.isJsonObject() || !"Arrange".equals(type(raw.getAsJsonObject()))) {
            return null;
        }
        JsonObject c = raw.getAsJsonObject();
        List<Integer> top = new ArrayList<>();
        List<Integer> bottom = new ArrayList<>();
        for (JsonElement e : c.getAsJsonArray("top")) {
            top.add(indexOfId(cards, e.getAsInt()));
        }
        for (JsonElement e : c.getAsJsonArray("bottom")) {
            bottom.add(indexOfId(cards, e.getAsInt()));
        }
        List<Integer> all = new ArrayList<>(top);
        all.addAll(bottom);
        return valid(all, cards.size(), cards.size(), cards.size(), false) ? List.of(top, bottom) : null;
    }

    /** {@code OrderTriggers}: Permutation der Indizes (erstes Element zuerst), oder null. */
    static List<Integer> order(Game game, int seat, List<String> descriptions, String prompt, List<Card> cardInfo) {
        if (descriptions.size() < 2) {
            List<Integer> one = new ArrayList<>();
            for (int i = 0; i < descriptions.size(); i++) {
                one.add(i);
            }
            return one;
        }
        JsonObject d = decision("OrderTriggers", seat);
        JsonArray arr = new JsonArray();
        descriptions.forEach(arr::add);
        d.add("descriptions", arr);
        // Erweiterung: Quellkarte je Eintrag (Trigger-Quelle bzw. die Karte selbst), damit die UI
        // die Einträge als Karten zeigen kann; -1 = keine
        if (cardInfo != null && cardInfo.size() == descriptions.size()) {
            JsonArray src = new JsonArray();
            cardInfo.forEach(c -> src.add(c == null ? -1 : c.getId()));
            d.add("sources", src);
        }
        JsonElement raw = askRaw(game, seat, d, prompt, null, null, false, null, cardInfo);
        List<Integer> order = raw != null && raw.isJsonObject() ? intList(raw.getAsJsonObject(), "Order", "order") : null;
        return valid(order, descriptions.size(), descriptions.size(), descriptions.size(), false) ? order : null;
    }

    /**
     * {@code AssignCombatDamage} (CR 510.1c–d): Schaden von {@code attacker} ({@code amount}) auf die
     * Blocker verteilen; {@code lethal[i]} = tödlicher Schaden für Blocker i, {@code trampleTo} =
     * angegriffener Spieler/Planeswalker bei Trampelschaden (sonst null). Gibt die Mengen je Blocker
     * zurück, bei Trampelschaden gefolgt von der Menge an {@code trampleTo}; null = unlesbar.
     */
    static List<Integer> assignDamage(Game game, int seat, Card attacker, int amount, List<Card> blockers,
            List<Integer> lethal, GameEntity trampleTo) {
        JsonObject d = decision("AssignCombatDamage", seat);
        d.addProperty("attacker", attacker.getId());
        d.addProperty("amount", amount);
        JsonArray bl = new JsonArray();
        blockers.forEach(b -> bl.add(b.getId()));
        d.add("blockers", bl);
        JsonArray le = new JsonArray();
        lethal.forEach(le::add);
        d.add("lethal", le);
        if (trampleTo != null) {
            d.add("trample_to", targetJson(game, trampleTo));
        }
        List<Card> info = new ArrayList<>(blockers);
        info.add(0, attacker);
        JsonElement raw = askRaw(game, seat, d, "Kampfschaden von " + attacker.getName() + " verteilen (" + amount + ")",
                null, attacker, false, null, info);
        List<Integer> values = raw != null && raw.isJsonObject() ? intList(raw.getAsJsonObject(), "Damage", "amounts") : null;
        int expected = blockers.size() + (trampleTo != null ? 1 : 0);
        if (values == null || values.size() < blockers.size() || values.size() > expected) {
            return null;
        }
        return values;
    }

    /**
     * Erweiterung {@code DivideAmount} (CR 601.2d): {@code amount} beim Wirken auf die gewählten
     * Ziele verteilen, jedes Ziel mindestens {@code min_each}. Gibt die Mengen je Ziel zurück oder
     * null (abgebrochen / unlesbar).
     */
    static List<Integer> divide(Game game, int seat, SpellAbility sa, int amount, List<GameEntity> targets,
            List<String> labels, int minEach) {
        JsonObject d = decision("DivideAmount", seat);
        d.addProperty("source", sa == null || sa.getHostCard() == null ? -1 : sa.getHostCard().getId());
        d.addProperty("amount", amount);
        JsonArray ts = new JsonArray();
        targets.forEach(t -> ts.add(targetJson(game, t)));
        d.add("targets", ts);
        d.addProperty("min_each", minEach);
        JsonObject c = ask(game, seat, d, amount + " verteilen", sa, null, true, labels);
        List<Integer> values = c == null ? null : intList(c, "Damage", "amounts");
        if (values == null || values.size() != targets.size()) {
            return null;
        }
        int sum = 0;
        for (int v : values) {
            if (v < minEach) {
                return null;
            }
            sum += v;
        }
        return sum == amount ? values : null;
    }

    /** {@code PayUnlessCost} (CR 118.12): Kosten zahlen, um den Effekt zu verhindern? */
    static Boolean payUnless(Game game, int seat, SpellAbility sa, String cost) {
        JsonObject d = decision("PayUnlessCost", seat);
        d.addProperty("cost", cost);
        JsonObject c = ask(game, seat, d, "Zahlen, um den Effekt zu verhindern: " + cost + "?", sa, null, false, null);
        return switch (type(c)) {
            case "Yes" -> true;
            case "No" -> false;
            default -> null;
        };
    }

    /** {@code PayLifeForPhyrexian}: phyrexianisches Symbol mit 2 Leben bezahlen? */
    static Boolean phyrexian(Game game, int seat, SpellAbility sa, String symbol) {
        JsonObject d = decision("PayLifeForPhyrexian", seat);
        d.addProperty("symbol", symbol);
        JsonObject c = ask(game, seat, d, "Phyrexianisches Mana " + symbol + " mit 2 Leben bezahlen?", sa, null, false, null);
        return switch (type(c)) {
            case "Yes" -> true;
            case "No" -> false;
            default -> null;
        };
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

    /**
     * Erweiterung {@code ChooseStartingPlayer} (CR 103.1): wer beginnt? {@code players} =
     * Sitzplätze zur Wahl, {@code labels} = Spielernamen; Antwort {@code Number(index)}.
     */
    static Integer startingPlayer(Game game, int seat, List<Integer> players, List<String> names) {
        JsonObject d = decision("ChooseStartingPlayer", seat);
        JsonArray arr = new JsonArray();
        players.forEach(arr::add);
        d.add("players", arr);
        Integer i = number(ask(game, seat, d, "Wer beginnt?", null, null, false, names));
        return i != null && i >= 0 && i < players.size() ? i : null;
    }

    /** Erweiterung {@code ChooseOptions}: {@code min}–{@code max} Indizes (genau eine → {@code ChooseOption}). */
    static List<Integer> options(Game game, int seat, String prompt, SpellAbility sa, Card card, List<Opt> options,
            int min, int max, boolean cancelable) {
        return options(game, seat, prompt, sa, card, options, min, max, cancelable, null);
    }

    /**
     * Wie {@link #options}, mit Erweiterung {@code purpose}: wofür die Wahl ist (z. B.
     * {@code "OptionalCost"} = optionale Kosten wie Kicker), damit die UI sie passend zeigt.
     */
    static List<Integer> options(Game game, int seat, String prompt, SpellAbility sa, Card card, List<Opt> options,
            int min, int max, boolean cancelable, String purpose) {
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
        if (purpose != null) {
            d.addProperty("purpose", purpose);
        }
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
