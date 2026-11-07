package clash.forgebridge.engine;

import java.util.ArrayList;
import java.util.List;

import forge.ai.AiCostDecision;
import forge.game.card.Card;
import forge.game.card.CardCollection;
import forge.game.card.CardLists;
import forge.game.card.CardPredicates;
import forge.game.cost.CostDiscard;
import forge.game.cost.CostExile;
import forge.game.cost.CostReturn;
import forge.game.cost.CostSacrifice;
import forge.game.cost.CostTapType;
import forge.game.cost.PaymentDecision;
import forge.game.player.Player;
import forge.game.spellability.SpellAbility;
import forge.game.zone.ZoneType;

/**
 * Kosten-Entscheider für den ferngesteuerten Spieler: Karten, die als <em>Kosten</em> geopfert,
 * abgeworfen, getappt, auf die Hand zurückgegeben oder exiliert werden, wählt der Spieler selbst
 * ({@code Decision::ChooseCards} mit Grund {@code SacrificeCost}, {@code DiscardCost},
 * {@code TapCost}, {@code ReturnCost}, {@code ExileCost}).
 *
 * Fahrzeuge besetzen (Gesamtstärke) wählt der Spieler ebenfalls.
 *
 * <p>Alles andere – Mana (geht ohnehin über {@code payManaCost} an den Spieler), Leben, Marken,
 * Sonderfälle ohne echte Wahl wie „Hand abwerfen“, „zufällig“, „Karte selbst“ – bleibt bei der
 * geerbten KI-Logik. Antwortet der Client ungültig, entscheidet ebenfalls die KI.
 */
final class RemoteCostDecision extends AiCostDecision {
    private final int seat;
    /** Wirken wurde abgebrochen: jede Kosten-Wahl lehnt ab, Forge nimmt das Wirken zurück. */
    private final boolean refuse;

    RemoteCostDecision(int seat, Player player, SpellAbility sa, boolean effect, boolean refuse) {
        super(player, sa, effect);
        this.seat = seat;
        this.refuse = refuse;
    }

    /** Fragt {@code amount} Karten aus {@code valid}; null = KI soll entscheiden. */
    private PaymentDecision ask(String reason, CardCollection valid, int amount, String prompt) {
        if (amount <= 0 || valid.size() < amount) {
            return null;
        }
        List<Card> list = new ArrayList<>(valid);
        List<Integer> chosen = Decisions.cards(player.getGame(), seat, reason, list, amount, amount, prompt, ability,
                false);
        if (chosen == null) {
            return null;
        }
        CardCollection result = new CardCollection();
        chosen.forEach(i -> result.add(list.get(i)));
        return PaymentDecision.card(result);
    }

    private CardCollection valid(ZoneType zone, String type) {
        return CardLists.getValidCards(player.getCardsIn(zone), type.split(";"), player, source, ability);
    }

    /**
     * „Eine Marke von einem eigenen Permanent entfernen“ (Power Conduit u. a.): der Spieler wählt
     * das Permanent auf dem Tisch und – bei mehreren Markenarten darauf – die Markenart
     * ({@code ChooseOption}, {@code purpose = "CounterType"}). Nur für eine Marke; sonst KI.
     */
    @Override
    public PaymentDecision visit(forge.game.cost.CostRemoveAnyCounter cost) {
        if (refuse) {
            return null;
        }
        int amount = cost.getAbilityAmount(ability);
        if (amount != 1) {
            return super.visit(cost);
        }
        CardCollection valid = cost.payCostFromSource() ? new CardCollection(ability.getHostCard())
                : valid(ZoneType.Battlefield, cost.getType());
        CardCollection withCounters = new CardCollection();
        for (Card c : valid) {
            boolean ok = cost.counter == null ? c.hasCounters() : c.getCounters(cost.counter) > 0;
            if (ok) {
                withCounters.add(c);
            }
        }
        if (withCounters.isEmpty()) {
            return null;
        }
        Card chosenCard;
        if (withCounters.size() == 1) {
            chosenCard = withCounters.getFirst();
        } else {
            List<Card> list = new ArrayList<>(withCounters);
            List<Integer> chosen = Decisions.cards(player.getGame(), seat, "Choose", list, 1, 1,
                    "Marke entfernen: Permanent wählen", ability, false);
            if (chosen == null || chosen.isEmpty()) {
                return super.visit(cost);
            }
            chosenCard = list.get(chosen.get(0));
        }
        forge.game.card.CounterType type = cost.counter;
        if (type == null) {
            List<forge.game.card.CounterType> types = new ArrayList<>();
            for (var e : chosenCard.getCounters().entrySet()) {
                if (e.getCount() > 0) {
                    types.add(e.getElement());
                }
            }
            if (types.isEmpty()) {
                return null;
            }
            type = types.get(0);
            if (types.size() > 1) {
                List<Decisions.Opt> opts = new ArrayList<>();
                for (forge.game.card.CounterType ct : types) {
                    String key = ct instanceof Enum<?> en ? en.name() : ct.getName();
                    opts.add(new Decisions.Opt(ct.getName(), null, -1, key));
                }
                Integer i = Decisions.option(player.getGame(), seat, "Welche Marke entfernen?", ability, chosenCard,
                        opts, false, "CounterType");
                if (i != null) {
                    type = types.get(i);
                }
            }
        }
        forge.game.GameEntityCounterTable table = new forge.game.GameEntityCounterTable();
        table.put(null, chosenCard, type, 1);
        return PaymentDecision.counters(table);
    }

    @Override
    public PaymentDecision visit(CostSacrifice cost) {
        if (refuse) {
            return null;
        }
        if (cost.payCostFromSource() || "OriginalHost".equals(cost.getType()) || "All".equals(cost.getAmount())) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        PaymentDecision d = ask("SacrificeCost", valid(ZoneType.Battlefield, cost.getType()), c,
                "Kosten: " + c + " bleibende Karte(n) opfern");
        return d != null ? d : super.visit(cost);
    }

    @Override
    public PaymentDecision visit(CostDiscard cost) {
        if (refuse) {
            return null;
        }
        String type = cost.getType();
        if (cost.payCostFromSource() || type.equals("LastDrawn") || type.equals("Hand") || type.equals("Random")
                || type.contains("WithSameName") || type.contains("WithDifferentNames")) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        PaymentDecision d = ask("DiscardCost", valid(ZoneType.Hand, type), c, "Kosten: " + c + " Karte(n) abwerfen");
        return d != null ? d : super.visit(cost);
    }

    @Override
    public PaymentDecision visit(CostTapType cost) {
        if (refuse) {
            return null;
        }
        String type = cost.getType();
        if (type.contains("+withTotalPowerGE")) {
            PaymentDecision d = crew(cost, type);
            return d != null ? d : super.visit(cost);
        }
        if (type.contains("sharesCreatureTypeWith")) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        CardCollection valid = CardLists.filter(valid(ZoneType.Battlefield, type),
                card -> card.canTap() && (cost.canTapSource || card != source));
        PaymentDecision d = ask("TapCost", valid, c, "Kosten: " + c + " Karte(n) tappen");
        return d != null ? d : super.visit(cost);
    }

    /**
     * Exil als Kosten (z. B. „Exile a card from your graveyard“). Sonderfälle bleiben bei der KI:
     * die Karte selbst, „alle“, oberste Friedhofskarten, Mindest-Manawert, aus der Bibliothek
     * und „aus demselben Friedhof“ (mehrere Spieler).
     */
    @Override
    public PaymentDecision visit(CostExile cost) {
        if (refuse) {
            return null;
        }
        String type = cost.getType();
        boolean libraryOnly = cost.getFrom().size() == 1 && cost.getFrom().get(0) == ZoneType.Library;
        if (cost.payCostFromSource() || type.equals("All") || type.contains("FromTopGrave")
                || type.contains("+withTotalCMCGE") || libraryOnly || cost.zoneRestriction == 0) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        CardCollection pool = new CardCollection(cost.zoneRestriction == 1
                ? player.getCardsIn(cost.getFrom()) : player.getGame().getCardsIn(cost.getFrom()));
        CardCollection valid = CardLists.filter(
                CardLists.getValidCards(pool, type.split(";"), player, source, ability),
                CardPredicates.canExiledBy(ability, isEffect()));
        PaymentDecision d = ask("ExileCost", valid, c, "Kosten: " + c + " Karte(n) exilieren");
        return d != null ? d : super.visit(cost);
    }

    /**
     * Besetzen (Crew, CR 702.122) u. Ä.: beliebig viele ungetappte Kreaturen mit zusammen
     * mindestens N Stärke tappen. Zu wenig Stärke → bis zu drei Mal erneut fragen, dann KI.
     */
    private PaymentDecision crew(CostTapType cost, String type) {
        int needed;
        try {
            needed = Integer.parseInt(type.split("withTotalPowerGE")[1].trim());
        } catch (NumberFormatException e) {
            return null;
        }
        String base = type.replace("+withTotalPowerGE" + type.split("withTotalPowerGE")[1], "");
        CardCollection valid = CardLists.filter(valid(ZoneType.Battlefield, base),
                card -> card.canTap() && (cost.canTapSource || card != source));
        int total = 0;
        for (Card c : valid) {
            total += Math.max(0, c.getNetPower());
        }
        if (valid.isEmpty() || total < needed) {
            return null;
        }
        List<Card> list = new ArrayList<>(valid);
        for (int attempt = 0; attempt < 3; attempt++) {
            List<Integer> chosen = Decisions.cards(player.getGame(), seat, "TapCost", list, 1, list.size(),
                    "Besetzen: Kreaturen mit zusammen mindestens " + needed + " Stärke tappen", ability, false);
            if (chosen == null) {
                return null;
            }
            CardCollection result = new CardCollection();
            int power = 0;
            for (int i : chosen) {
                result.add(list.get(i));
                power += Math.max(0, list.get(i).getNetPower());
            }
            if (power >= needed) {
                return PaymentDecision.card(result);
            }
        }
        return null;
    }

    @Override
    public PaymentDecision visit(CostReturn cost) {
        if (refuse) {
            return null;
        }
        if (cost.payCostFromSource()) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        PaymentDecision d = ask("ReturnCost", valid(ZoneType.Battlefield, cost.getType()), c,
                "Kosten: " + c + " bleibende Karte(n) auf die Hand zurückgeben");
        return d != null ? d : super.visit(cost);
    }
}
