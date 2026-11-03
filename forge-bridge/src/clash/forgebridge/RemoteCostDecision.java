package clash.forgebridge;

import java.util.ArrayList;
import java.util.List;

import forge.ai.AiCostDecision;
import forge.game.card.Card;
import forge.game.card.CardCollection;
import forge.game.card.CardLists;
import forge.game.cost.CostDiscard;
import forge.game.cost.CostReturn;
import forge.game.cost.CostSacrifice;
import forge.game.cost.CostTapType;
import forge.game.cost.PaymentDecision;
import forge.game.player.Player;
import forge.game.spellability.SpellAbility;
import forge.game.zone.ZoneType;

/**
 * Kosten-Entscheider für den ferngesteuerten Spieler: Karten, die als <em>Kosten</em> geopfert,
 * abgeworfen, getappt oder auf die Hand zurückgegeben werden, wählt der Spieler selbst
 * ({@code Decision::ChooseCards} mit Grund {@code SacrificeCost}, {@code DiscardCost},
 * {@code TapCost}, {@code ReturnCost}).
 *
 * <p>Alles andere – Mana (geht ohnehin über {@code payManaCost} an den Spieler), Leben, Marken,
 * Sonderfälle wie „Hand abwerfen“, „zufällig“, Fahrzeuge (Gesamtstärke), „Karte selbst“ – bleibt
 * bei der geerbten KI-Logik. Antwortet der Client ungültig, entscheidet ebenfalls die KI.
 */
final class RemoteCostDecision extends AiCostDecision {
    private final int seat;

    RemoteCostDecision(int seat, Player player, SpellAbility sa, boolean effect) {
        super(player, sa, effect);
        this.seat = seat;
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

    @Override
    public PaymentDecision visit(CostSacrifice cost) {
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
        String type = cost.getType();
        if (type.contains("withTotalPowerGE") || type.contains("sharesCreatureTypeWith")) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        CardCollection valid = CardLists.filter(valid(ZoneType.Battlefield, type),
                card -> card.canTap() && (cost.canTapSource || card != source));
        PaymentDecision d = ask("TapCost", valid, c, "Kosten: " + c + " Karte(n) tappen");
        return d != null ? d : super.visit(cost);
    }

    @Override
    public PaymentDecision visit(CostReturn cost) {
        if (cost.payCostFromSource()) {
            return super.visit(cost);
        }
        int c = cost.getAbilityAmount(ability);
        PaymentDecision d = ask("ReturnCost", valid(ZoneType.Battlefield, cost.getType()), c,
                "Kosten: " + c + " bleibende Karte(n) auf die Hand zurückgeben");
        return d != null ? d : super.visit(cost);
    }
}
