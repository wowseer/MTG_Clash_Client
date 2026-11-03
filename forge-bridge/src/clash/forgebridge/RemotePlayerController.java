package clash.forgebridge;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import com.google.common.collect.ListMultimap;

import forge.LobbyPlayer;
import forge.StaticData;
import forge.card.ColorSet;
import forge.card.ICardFace;
import forge.card.MagicColor;
import forge.ai.AiPlayDecision;
import forge.ai.ComputerUtilMana;
import forge.ai.PlayerControllerAi;
import forge.game.Game;
import forge.game.GameEntity;
import forge.game.card.Card;
import forge.game.card.CardState;
import forge.game.card.CounterType;
import forge.game.cost.Cost;
import forge.game.keyword.KeywordInterface;
import forge.game.player.Player;
import forge.game.player.PlayerActionConfirmMode;
import forge.game.replacement.ReplacementEffect;
import forge.game.spellability.AbilitySub;
import forge.game.spellability.OptionalCostValue;
import forge.game.spellability.SpellAbility;
import forge.game.staticability.StaticAbility;
import forge.game.trigger.WrappedAbility;
import forge.game.zone.ZoneType;

/**
 * Spielersteuerung, deren Hauptentscheidungen MTG Clash trifft.
 *
 * <p>Ferngesteuert sind: Priorität (welche Karte/Fähigkeit spielen oder passen), Mulligan und
 * alle allgemeinen Auswahl-Dialoge (Optionen, Modi, Ja/Nein, Zahlen, Farben, Kartennamen; siehe
 * {@link RemoteChoice}). Ziele, Manabezahlung, Angreifer, Blocker und Kartenauswahl trifft
 * vorerst die Forge-KI, von der diese Klasse erbt. Antwortet der Client ungültig, entscheidet
 * ebenfalls die KI. Weitere
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
     * Ist der gewählte Zug nicht ausführbar (nicht spielbar, nicht bezahlbar, kein gültiges Ziel),
     * kommt "rejected" und es wird erneut gefragt. Ob die KI den Zug für sinnvoll hält, spielt
     * keine Rolle.
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
            // canPlaySa setzt bei gezielten Zaubern auch die Ziele (vorerst wählt die KI sie).
            // Ihre Meinung, ob der Zug sinnvoll ist, zählt nicht: abgelehnt wird nur, was
            // wirklich nicht geht (nicht spielbar, nicht bezahlbar, kein gültiges Ziel).
            AiPlayDecision decision = getAi().canPlaySa(sa);
            if (isPlayable(sa, decision)) {
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

    private static boolean isPlayable(SpellAbility sa, AiPlayDecision decision) {
        if (decision != null && decision.willingToPlay()) {
            return true;
        }
        if (decision == AiPlayDecision.CantPlaySa || decision == AiPlayDecision.CantAfford
                || decision == AiPlayDecision.CantAffordX) {
            return false;
        }
        return !sa.usesTargeting() || sa.isTargetNumberValid();
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

    // ------------------------------------------------------------------ Auswahl-Dialoge

    private Game g() {
        return getGame();
    }

    private static String describe(SpellAbility sa) {
        String d = sa.getDescription();
        return d == null || d.isBlank() ? sa.toUnsuppressedString() : d;
    }

    private static <T> T pick(List<T> items, Integer index) {
        return index == null ? null : items.get(index);
    }

    /** Modi eines modalen Zaubers ("Wähle eins –"). */
    @Override
    public List<AbilitySub> chooseModeForAbility(SpellAbility sa, List<AbilitySub> possible, int min, int num,
            boolean allowRepeat) {
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (AbilitySub sub : possible) {
            opts.add(new RemoteChoice.Option(describe(sub), null));
        }
        String title = min == num ? (num == 1 ? "Wähle einen Modus" : "Wähle " + num + " Modi")
                : "Wähle " + min + " bis " + num + " Modi";
        List<Integer> chosen = RemoteChoice.multi(g(), seat, title, sa, null, opts, min, num, min == 0);
        if (chosen == null) {
            return super.chooseModeForAbility(sa, possible, min, num, allowRepeat);
        }
        List<AbilitySub> result = new ArrayList<>();
        for (int i : chosen) {
            result.add(possible.get(i));
        }
        return result;
    }

    /** Optionale Zusatzkosten (Kicker, Verstärkung …). */
    @Override
    public List<OptionalCostValue> chooseOptionalCosts(SpellAbility sa, List<OptionalCostValue> optionalCost) {
        List<Integer> chosen = RemoteChoice.multi(g(), seat, "Optionale Kosten zahlen?", sa, null,
                RemoteChoice.options(optionalCost), 0, optionalCost.size(), false);
        if (chosen == null) {
            return super.chooseOptionalCosts(sa, optionalCost);
        }
        List<OptionalCostValue> result = new ArrayList<>();
        for (int i : chosen) {
            result.add(optionalCost.get(i));
        }
        return result;
    }

    @Override
    public String chooseSomeType(String kindOfType, SpellAbility sa, Collection<String> validTypes,
            boolean isOptional) {
        List<String> types = new ArrayList<>(validTypes);
        String r = pick(types, RemoteChoice.single(g(), seat, "Wähle: " + kindOfType, sa, null,
                RemoteChoice.options(types), isOptional));
        return r != null ? r : isOptional ? null : super.chooseSomeType(kindOfType, sa, validTypes, isOptional);
    }

    @Override
    public String chooseSector(Card assignee, String ai, List<String> sectors) {
        String r = pick(sectors, RemoteChoice.single(g(), seat, "Wähle einen Sektor", null, assignee,
                RemoteChoice.options(sectors), false));
        return r != null ? r : super.chooseSector(assignee, ai, sectors);
    }

    @Override
    public String chooseKeywordForPump(List<String> options, SpellAbility sa, String prompt, Card tgtCard) {
        String r = pick(options, RemoteChoice.single(g(), seat, prompt, sa, tgtCard,
                RemoteChoice.options(options), false));
        return r != null ? r : super.chooseKeywordForPump(options, sa, prompt, tgtCard);
    }

    @Override
    public ReplacementEffect chooseSingleReplacementEffect(List<ReplacementEffect> possible) {
        ReplacementEffect r = pick(possible, RemoteChoice.single(g(), seat, "Welcher Ersatzeffekt gilt zuerst?",
                null, null, RemoteChoice.options(possible), false));
        return r != null ? r : super.chooseSingleReplacementEffect(possible);
    }

    @Override
    public StaticAbility chooseSingleStaticAbility(List<StaticAbility> possible) {
        StaticAbility r = pick(possible, RemoteChoice.single(g(), seat, "Welche statische Fähigkeit gilt zuerst?",
                null, null, RemoteChoice.options(possible), false));
        return r != null ? r : super.chooseSingleStaticAbility(possible);
    }

    @Override
    public String chooseProtectionType(SpellAbility sa, List<String> choices) {
        String r = pick(choices, RemoteChoice.single(g(), seat, "Schutz wovor?", sa, null,
                RemoteChoice.options(choices), false));
        return r != null ? r : super.chooseProtectionType(sa, choices);
    }

    @Override
    public CounterType chooseCounterType(List<CounterType> options, SpellAbility sa, String prompt,
            Map<String, Object> params) {
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (CounterType c : options) {
            opts.add(new RemoteChoice.Option(c.getName(), null));
        }
        CounterType r = pick(options, RemoteChoice.single(g(), seat, prompt, sa, null, opts, false));
        return r != null ? r : super.chooseCounterType(options, sa, prompt, params);
    }

    @Override
    public ICardFace chooseSingleCardFace(SpellAbility sa, List<ICardFace> faces, String message) {
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (ICardFace f : faces) {
            opts.add(new RemoteChoice.Option(f.getName(), null));
        }
        ICardFace r = pick(faces, RemoteChoice.single(g(), seat, message, sa, null, opts, false));
        return r != null ? r : super.chooseSingleCardFace(sa, faces, message);
    }

    @Override
    public CardState chooseSingleCardState(SpellAbility sa, List<CardState> states, String message,
            Map<String, Object> params) {
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (CardState st : states) {
            opts.add(new RemoteChoice.Option(st.getName(), null));
        }
        CardState r = pick(states, RemoteChoice.single(g(), seat, message, sa, null, opts, false));
        return r != null ? r : super.chooseSingleCardState(sa, states, message, params);
    }

    @Override
    public Player chooseStartingPlayer(boolean isFirstGame) {
        List<Player> players = new ArrayList<>(g().getPlayers());
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (Player p : players) {
            opts.add(new RemoteChoice.Option(p == getPlayer() ? p.getName() + " (du)" : p.getName(), null));
        }
        Player r = pick(players, RemoteChoice.single(g(), seat, "Wer beginnt?", null, null, opts, false));
        return r != null ? r : super.chooseStartingPlayer(isFirstGame);
    }

    @Override
    public Object vote(SpellAbility sa, String prompt, List<Object> options, ListMultimap<Object, Player> votes,
            Player forPlayer, boolean optional) {
        Object r = pick(options, RemoteChoice.single(g(), seat, prompt, sa, null, RemoteChoice.options(options),
                optional));
        return r != null ? r : optional ? null : super.vote(sa, prompt, options, votes, forPlayer, optional);
    }

    @Override
    public Integer chooseRollToIgnore(List<Integer> rolls) {
        Integer r = pick(rolls, RemoteChoice.single(g(), seat, "Welchen Wurf ignorieren?", null, null,
                RemoteChoice.options(rolls), false));
        return r != null ? r : super.chooseRollToIgnore(rolls);
    }

    @Override
    public Integer chooseRollToModify(List<Integer> rolls) {
        Integer r = pick(rolls, RemoteChoice.single(g(), seat, "Welchen Wurf verändern?", null, null,
                RemoteChoice.options(rolls), true));
        return r;
    }

    @Override
    public List<Integer> chooseDiceToReroll(List<Integer> rolls) {
        List<Integer> chosen = RemoteChoice.multi(g(), seat, "Welche Würfel neu werfen?", null, null,
                RemoteChoice.options(rolls), 0, rolls.size(), false);
        if (chosen == null) {
            return super.chooseDiceToReroll(rolls);
        }
        List<Integer> result = new ArrayList<>();
        for (int i : chosen) {
            result.add(rolls.get(i));
        }
        return result;
    }

    @Override
    public int chooseSprocket(Card assignee, List<Integer> sprockets) {
        Integer r = pick(sprockets, RemoteChoice.single(g(), seat, "Wähle ein Zahnrad", null, assignee,
                RemoteChoice.options(sprockets), false));
        return r != null ? r : super.chooseSprocket(assignee, sprockets);
    }

    // ------------------------------------------------------------------ Ja/Nein

    @Override
    public boolean confirmAction(SpellAbility sa, PlayerActionConfirmMode mode, String message,
            List<String> options, Card cardToShow, Map<String, Object> params) {
        String yes = options != null && options.size() == 2 ? options.get(0) : "Ja";
        String no = options != null && options.size() == 2 ? options.get(1) : "Nein";
        Boolean r = RemoteChoice.confirm(g(), seat, message, sa, cardToShow, yes, no);
        return r != null ? r : super.confirmAction(sa, mode, message, options, cardToShow, params);
    }

    @Override
    public boolean confirmTrigger(WrappedAbility wrapper) {
        Boolean r = RemoteChoice.confirm(g(), seat, "Ausgelöste Fähigkeit nutzen?\n" + wrapper.toUnsuppressedString(),
                wrapper, wrapper.getHostCard(), "Ja", "Nein");
        return r != null ? r : super.confirmTrigger(wrapper);
    }

    @Override
    public boolean confirmReplacementEffect(ReplacementEffect replacementEffect, SpellAbility effectSA,
            GameEntity affected, String question) {
        Boolean r = RemoteChoice.confirm(g(), seat, question, effectSA, replacementEffect.getHostCard(), "Ja", "Nein");
        return r != null ? r : super.confirmReplacementEffect(replacementEffect, effectSA, affected, question);
    }

    @Override
    public boolean confirmStaticApplication(Card hostCard, PlayerActionConfirmMode mode, String message,
            String logic) {
        Boolean r = RemoteChoice.confirm(g(), seat, message, null, hostCard, "Ja", "Nein");
        return r != null ? r : super.confirmStaticApplication(hostCard, mode, message, logic);
    }

    @Override
    public boolean chooseBinary(SpellAbility sa, String question, BinaryChoiceType kindOfChoice, Boolean defaultVal) {
        String[] labels = switch (kindOfChoice) {
            case HeadsOrTails -> new String[] {"Kopf", "Zahl"};
            case TapOrUntap -> new String[] {"Tappen", "Enttappen"};
            case PlayOrDraw -> new String[] {"Beginnen", "Ziehen"};
            case OddsOrEvens -> new String[] {"Ungerade", "Gerade"};
            case UntapOrLeaveTapped -> new String[] {"Enttappen", "Getappt lassen"};
            case LeftOrRight -> new String[] {"Links", "Rechts"};
            case AddOrRemove -> new String[] {"Hinzufügen", "Entfernen"};
            case IncreaseOrDecrease -> new String[] {"Erhöhen", "Verringern"};
        };
        Boolean r = RemoteChoice.confirm(g(), seat, question, sa, null, labels[0], labels[1]);
        return r != null ? r : super.chooseBinary(sa, question, kindOfChoice, defaultVal);
    }

    @Override
    public boolean chooseFlipResult(SpellAbility sa, Player flipper, boolean call) {
        Boolean r = call
                ? RemoteChoice.confirm(g(), seat, "Kopf oder Zahl?", sa, null, "Kopf", "Zahl")
                : RemoteChoice.confirm(g(), seat, "Ergebnis des Münzwurfs wählen", sa, null, "Gewinnen", "Verlieren");
        return r != null ? r : super.chooseFlipResult(sa, flipper, call);
    }

    @Override
    public boolean willPutCardOnTop(Card c) {
        Boolean r = RemoteChoice.confirm(g(), seat, "Karte oben auf die Bibliothek legen?", null, c, "Oben",
                "Unten");
        return r != null ? r : super.willPutCardOnTop(c);
    }

    // ------------------------------------------------------------------ Zahlen

    @Override
    public int chooseNumber(SpellAbility sa, String title, int min, int max) {
        Integer r = RemoteChoice.number(g(), seat, title, sa, null, min, max, false);
        return r != null ? r : super.chooseNumber(sa, title, min, max);
    }

    @Override
    public int chooseNumber(SpellAbility sa, String title, List<Integer> values, Player relatedPlayer) {
        Integer r = pick(values, RemoteChoice.single(g(), seat, title, sa, null, RemoteChoice.options(values),
                false));
        return r != null ? r : super.chooseNumber(sa, title, values, relatedPlayer);
    }

    /** X-Kosten und andere angesagte Werte beim Spielen. Abbrechen = Zauber nicht spielen. */
    @Override
    public Integer announceRequirements(SpellAbility ability, int min, int max, String announce) {
        return RemoteChoice.number(g(), seat, "Wert für " + announce + " wählen", ability, null, min, max, true);
    }

    @Override
    public int chooseNumberForCostReduction(SpellAbility sa, int min, int max) {
        Integer r = RemoteChoice.number(g(), seat, "Um wie viel die Kosten verringern?", sa, null, min, max, false);
        return r != null ? r : super.chooseNumberForCostReduction(sa, min, max);
    }

    @Override
    public int chooseNumberForKeywordCost(SpellAbility sa, Cost cost, KeywordInterface keyword, String prompt,
            int max) {
        Integer r = RemoteChoice.number(g(), seat, prompt, sa, null, 0, max, false);
        return r != null ? r : super.chooseNumberForKeywordCost(sa, cost, keyword, prompt, max);
    }

    // ------------------------------------------------------------------ Farben

    private static List<RemoteChoice.Option> colorOptions(List<MagicColor.Color> colors) {
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (MagicColor.Color c : colors) {
            opts.add(new RemoteChoice.Option(c.getTranslatedName(), c.getShortName().toUpperCase()));
        }
        return opts;
    }

    @Override
    public byte chooseColor(String message, SpellAbility sa, ColorSet colors) {
        List<MagicColor.Color> list = new ArrayList<>(colors.getOrderedColors());
        MagicColor.Color r = pick(list, RemoteChoice.single(g(), seat, message, sa, null, colorOptions(list), false));
        return r != null ? r.getColorMask() : super.chooseColor(message, sa, colors);
    }

    @Override
    public byte chooseColorAllowColorless(String message, Card c, ColorSet colors) {
        List<MagicColor.Color> list = new ArrayList<>(colors.getOrderedColors());
        List<RemoteChoice.Option> opts = colorOptions(list);
        opts.add(new RemoteChoice.Option("Farblos", "C"));
        Integer i = RemoteChoice.single(g(), seat, message, null, c, opts, false);
        if (i == null) {
            return super.chooseColorAllowColorless(message, c, colors);
        }
        return i == list.size() ? MagicColor.COLORLESS : list.get(i).getColorMask();
    }

    @Override
    public ColorSet chooseColors(String message, SpellAbility sa, int min, int max, ColorSet options) {
        List<MagicColor.Color> list = new ArrayList<>(options.getOrderedColors());
        List<Integer> chosen = RemoteChoice.multi(g(), seat, message, sa, null, colorOptions(list), min, max, false);
        if (chosen == null) {
            return super.chooseColors(message, sa, min, max, options);
        }
        int mask = 0;
        for (int i : chosen) {
            mask |= list.get(i).getColorMask();
        }
        return ColorSet.fromMask(mask);
    }

    // ------------------------------------------------------------------ Kartennamen

    @Override
    public String chooseCardName(SpellAbility sa, List<ICardFace> faces, String message) {
        List<RemoteChoice.Option> opts = new ArrayList<>();
        for (ICardFace f : faces) {
            opts.add(new RemoteChoice.Option(f.getName(), null));
        }
        ICardFace r = pick(faces, RemoteChoice.single(g(), seat, message, sa, null, opts, false));
        return r != null ? r.getName() : super.chooseCardName(sa, faces, message);
    }

    /**
     * Kartenname mit Bedingung: Bei wenigen erlaubten Karten (≤ 60) als Liste, sonst als
     * Texteingabe; ungültige Namen werden mit Fehlermeldung erneut erfragt (höchstens 5-mal).
     */
    @Override
    public String chooseCardName(SpellAbility sa, Predicate<ICardFace> cpp, String valid, String message) {
        List<ICardFace> allowed = new ArrayList<>();
        for (ICardFace f : StaticData.instance().getCommonCards().getAllFaces()) {
            if (cpp.test(f)) {
                allowed.add(f);
                if (allowed.size() > 60) {
                    break;
                }
            }
        }
        if (!allowed.isEmpty() && allowed.size() <= 60) {
            return chooseCardName(sa, allowed, message);
        }
        String error = null;
        for (int attempt = 0; attempt < 5; attempt++) {
            String name = RemoteChoice.text(g(), seat, message, sa, null, error, false);
            if (name == null) {
                break;
            }
            ICardFace face = StaticData.instance().getCommonCards().getFaceByName(name);
            if (face != null && cpp.test(face)) {
                return face.getName();
            }
            error = face == null ? "Unbekannte Karte: " + name : "Diese Karte ist hier nicht erlaubt: " + name;
        }
        return super.chooseCardName(sa, cpp, valid, message);
    }
}
