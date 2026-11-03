package clash.forgebridge;

import java.util.ArrayList;
import java.util.List;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import forge.LobbyPlayer;
import forge.ai.AiPlayDecision;
import forge.ai.ComputerUtilMana;
import forge.ai.PlayerControllerAi;
import forge.game.Game;
import forge.game.card.Card;
import forge.game.player.Player;
import forge.game.spellability.SpellAbility;
import forge.game.zone.ZoneType;

/**
 * Spielersteuerung, deren Hauptentscheidungen MTG Clash trifft.
 *
 * <p>Ferngesteuert sind: Priorität (welche Karte/Fähigkeit spielen oder passen) und
 * Mulligan. Alle übrigen Entscheidungen (Ziele, Manabezahlung, Angreifer, Blocker,
 * Auswahl-Dialoge …) trifft vorerst die Forge-KI, von der diese Klasse erbt. Weitere
 * Entscheidungen lassen sich ferngesteuert machen, indem man die passende Methode von
 * {@code PlayerController} überschreibt und {@link ForgeBridge#request} aufruft.
 */
public class RemotePlayerController extends PlayerControllerAi {
    private static final ZoneType[] PLAY_ZONES = {
        ZoneType.Hand, ZoneType.Battlefield, ZoneType.Graveyard, ZoneType.Exile, ZoneType.Command,
    };

    private final int seat;

    public RemotePlayerController(Game game, Player player, LobbyPlayer lobby) {
        super(game, player, lobby);
        this.seat = lobby instanceof RemoteLobbyPlayer r ? r.getSeat() : -1;
    }

    /**
     * Alle Fähigkeiten, die der Spieler jetzt spielen darf und bezahlen kann
     * (ohne Manafähigkeiten – Mana wird beim Ausführen automatisch bezahlt).
     */
    private List<SpellAbility> playableOptions() {
        Player me = getPlayer();
        List<SpellAbility> options = new ArrayList<>();
        for (ZoneType zone : PLAY_ZONES) {
            for (Card card : me.getCardsIn(zone)) {
                for (SpellAbility sa : card.getAllPossibleAbilities(me, true)) {
                    if (sa.isManaAbility()) {
                        continue;
                    }
                    sa.setActivatingPlayer(me);
                    if (sa.isLandAbility() || ComputerUtilMana.canPayManaCost(sa, me, 0, false)) {
                        options.add(sa);
                    }
                }
            }
        }
        return options;
    }

    /**
     * Priorität: fragt den Client mit "request"/kind "priority". Antwort "choice" = Index in
     * "options" oder -1 / null = passen. Gibt es nichts zu spielen, wird automatisch gepasst.
     * Lehnt die Forge-KI das Ausführen ab (z. B. keine gültigen Ziele), kommt "rejected" und
     * es wird erneut gefragt.
     */
    @Override
    public List<SpellAbility> chooseSpellAbilityToPlay() {
        while (true) {
            List<SpellAbility> options = playableOptions();
            if (options.isEmpty()) {
                return null;
            }
            JsonObject payload = new JsonObject();
            JsonArray arr = new JsonArray();
            for (int i = 0; i < options.size(); i++) {
                arr.add(StateJson.ability(i, options.get(i)));
            }
            payload.add("options", arr);
            payload.add("state", StateJson.snapshot(getGame(), seat));

            JsonElement answer = ForgeBridge.request("priority", seat, payload);
            int choice = answer != null && answer.isJsonPrimitive() ? answer.getAsInt() : -1;
            if (choice < 0 || choice >= options.size()) {
                return null; // passen
            }
            SpellAbility sa = options.get(choice);
            AiPlayDecision decision = getAi().canPlaySa(sa);
            if (decision != null && decision.willingToPlay()) {
                List<SpellAbility> result = new ArrayList<>();
                result.add(sa);
                return result;
            }
            JsonObject rejected = ForgeBridge.message("rejected");
            rejected.addProperty("player", seat);
            rejected.addProperty("option", choice);
            rejected.addProperty("reason", String.valueOf(decision));
            ForgeBridge.send(rejected);
        }
    }

    /** Mulligan: "request"/kind "mulligan"; Antwort true = behalten, false = Mulligan. */
    @Override
    public boolean mulliganKeepHand(Player firstPlayer, int cardsToReturn) {
        JsonObject payload = new JsonObject();
        payload.addProperty("cards_to_return", cardsToReturn);
        payload.add("state", StateJson.snapshot(getGame(), seat));
        JsonElement answer = ForgeBridge.request("mulligan", seat, payload);
        return answer == null || !answer.isJsonPrimitive() || answer.getAsBoolean();
    }
}
