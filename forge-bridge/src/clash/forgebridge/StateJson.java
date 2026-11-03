package clash.forgebridge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import forge.card.MagicColor;
import forge.game.Game;
import forge.game.GameEntity;
import forge.game.card.Card;
import forge.game.combat.Combat;
import forge.game.phase.PhaseHandler;
import forge.game.player.Player;
import forge.game.spellability.SpellAbility;
import forge.game.zone.ZoneType;

/** Wandelt Forge-Spielzustand in JSON für den Client um. */
final class StateJson {
    private StateJson() {}

    /**
     * Momentaufnahme des Spiels. {@code viewer} = Sitzplatz, dessen Handkarten offen
     * mitgeschickt werden (-1 = keine Hand offen; Gegnerhände nur als Anzahl).
     */
    static JsonObject snapshot(Game game, int viewer) {
        JsonObject s = new JsonObject();
        PhaseHandler ph = game.getPhaseHandler();
        s.addProperty("turn", ph.getTurn());
        s.addProperty("phase", ph.getPhase() == null ? null : ph.getPhase().name());
        Player active = ph.getPlayerTurn();
        s.addProperty("active_player", active == null ? -1 : indexOf(game, active));

        JsonArray players = new JsonArray();
        int i = 0;
        for (Player p : game.getPlayers()) {
            JsonObject po = new JsonObject();
            po.addProperty("seat", i);
            po.addProperty("name", p.getName());
            po.addProperty("life", p.getLife());
            po.addProperty("library", p.getCardsIn(ZoneType.Library).size());
            po.addProperty("hand_count", p.getCardsIn(ZoneType.Hand).size());
            // Manapool: Menge je Farbe in der Reihenfolge W U B R G C
            JsonArray pool = new JsonArray();
            for (byte color : MagicColor.WUBRGC) {
                pool.add(p.getManaPool().getAmountOfColor(color));
            }
            po.add("mana_pool", pool);
            if (i == viewer) {
                po.add("hand", cards(p, ZoneType.Hand));
            }
            po.add("battlefield", cards(p, ZoneType.Battlefield));
            po.add("graveyard", cards(p, ZoneType.Graveyard));
            po.add("exile", cards(p, ZoneType.Exile));
            players.add(po);
            i++;
        }
        s.add("players", players);
        s.add("combat", combat(game));
        return s;
    }

    /**
     * Laufender Kampf: {@code attackers} = Liste {@code {card, target}} (target = {@code {"card": id}}
     * für Planeswalker/Schlachten oder {@code {"player": sitz}}), {@code blocks} = Liste
     * {@code [blocker, angreifer]}. Ohne Kampf leere Listen.
     */
    static JsonObject combat(Game game) {
        JsonObject o = new JsonObject();
        JsonArray attackers = new JsonArray();
        JsonArray blocks = new JsonArray();
        Combat combat = game.getPhaseHandler().getCombat();
        if (combat != null) {
            for (Card a : combat.getAttackers()) {
                JsonObject ao = new JsonObject();
                ao.addProperty("card", a.getId());
                GameEntity def = combat.getDefenderByAttacker(a);
                if (def != null) {
                    ao.add("target", Decisions.targetJson(game, def));
                }
                attackers.add(ao);
                for (Card b : combat.getBlockers(a)) {
                    JsonArray pair = new JsonArray();
                    pair.add(b.getId());
                    pair.add(a.getId());
                    blocks.add(pair);
                }
            }
        }
        o.add("attackers", attackers);
        o.add("blocks", blocks);
        return o;
    }

    static JsonArray cards(Player p, ZoneType zone) {
        JsonArray arr = new JsonArray();
        for (Card c : p.getCardsIn(zone)) {
            arr.add(card(c));
        }
        return arr;
    }

    static JsonObject card(Card c) {
        JsonObject o = new JsonObject();
        o.addProperty("id", c.getId());
        o.addProperty("name", c.getName());
        o.addProperty("type", String.valueOf(c.getType()));
        o.addProperty("tapped", c.isTapped());
        o.addProperty("cost", c.getManaCost() == null ? "" : c.getManaCost().getShortString());
        if (c.isCreature()) {
            o.addProperty("power", c.getNetPower());
            o.addProperty("toughness", c.getNetToughness());
        }
        return o;
    }

    static JsonObject ability(int index, SpellAbility sa) {
        JsonObject o = new JsonObject();
        o.addProperty("index", index);
        Card host = sa.getHostCard();
        o.addProperty("card_id", host == null ? -1 : host.getId());
        o.addProperty("card", host == null ? null : host.getName());
        o.addProperty("land", sa.isLandAbility());
        o.addProperty("spell", sa.isSpell());
        o.addProperty("text", sa.toUnsuppressedString());
        return o;
    }

    static int indexOf(Game game, Player player) {
        int i = 0;
        for (Player p : game.getPlayers()) {
            if (p == player) {
                return i;
            }
            i++;
        }
        return -1;
    }
}
