package clash.forgebridge;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.HashMap;
import java.util.Map;
import java.util.TreeSet;
import java.util.function.Predicate;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import com.google.common.collect.ListMultimap;
import org.apache.commons.lang3.Range;

import forge.LobbyPlayer;
import forge.StaticData;
import forge.card.CardType;
import forge.card.ColorSet;
import forge.card.ICardFace;
import forge.card.MagicColor;
import forge.ai.AiPlayDecision;
import forge.ai.ComputerUtilMana;
import forge.ai.PlayerControllerAi;
import forge.game.Game;
import forge.game.GameActionUtil;
import forge.game.GameEntity;
import forge.game.ability.AbilityUtils;
import forge.game.ability.ApiType;
import forge.game.ability.effects.CharmEffect;
import forge.game.card.Card;
import forge.game.card.CardState;
import forge.game.card.CounterType;
import forge.game.combat.Combat;
import forge.game.combat.CombatUtil;
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
 * <p>Jede Frage geht als NeoForge-{@code Decision} an den Client ({@link Decisions},
 * Protokoll v3): Priorität, Mulligan, beim Wirken optionale Kosten, Modi, X und Ziele, dazu alle
 * allgemeinen Auswahl-Dialoge, Angreifer und Blocker. Manabezahlung und Kartenauswahl trifft vorerst
 * die Forge-KI, von der diese Klasse erbt. Antwortet der Client ungültig, entscheidet ebenfalls
 * die KI. Weitere Entscheidungen lassen sich ferngesteuert machen, indem man die passende Methode
 * von {@code PlayerController} überschreibt und die passende Methode von {@link Decisions} aufruft.
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
     * Alle Fähigkeiten, die der Spieler jetzt spielen darf und bezahlen kann und für die es
     * genug mögliche Ziele gibt (ohne Manafähigkeiten – Mana wird beim Ausführen automatisch
     * bezahlt).
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
                    if (!sa.isLandAbility() && !hasPossibleTargets(sa)) {
                        continue; // braucht ein Ziel, aber es gibt keins
                    }
                    if (sa.isLandAbility() || ComputerUtilMana.canPayManaCost(sa, me, 0, false)) {
                        options.add(sa);
                    }
                }
            }
        }
        return options;
    }

    /**
     * Priorität: {@code Decision::Priority} mit allen spielbaren Aktionen ({@code PlayLand},
     * {@code Cast}, {@code CastAlt}, {@code Activate} …) und {@code Pass}. Gibt es nichts zu
     * spielen, wird automatisch gepasst. Ist die gewählte Aktion nicht ausführbar (abgebrochen,
     * kein gültiges Ziel), kommt "rejected" und es wird erneut gefragt. Ob die KI den Zug für
     * sinnvoll hält, spielt keine Rolle.
     */
    @Override
    public List<SpellAbility> chooseSpellAbilityToPlay() {
        while (true) {
            List<SpellAbility> options = playableOptions();
            if (options.isEmpty()) {
                return null;
            }
            JsonArray actions = new JsonArray();
            List<String> labels = new ArrayList<>();
            JsonObject pass = new JsonObject();
            pass.addProperty("type", "Pass");
            actions.add(pass);
            labels.add("Passen");
            Map<String, Integer> counters = new HashMap<>();
            for (SpellAbility sa : options) {
                actions.add(actionJson(sa, counters));
                labels.add(sa.toUnsuppressedString());
            }
            JsonObject action = Decisions.priority(getGame(), seat, actions, labels);
            int choice = action == null ? 0 : indexOf(actions, action);
            if (choice <= 0) {
                return null; // passen (oder unlesbare Antwort)
            }
            choice -= 1;
            SpellAbility sa = options.get(choice);
            String reason;
            if (sa.isDividedAsYouChoose()) {
                // Schaden frei verteilen kommt später (Muster M8): Ziele und Verteilung von der KI.
                // Ihre Meinung, ob der Zug sinnvoll ist, zählt nicht: abgelehnt wird nur, was
                // wirklich nicht geht (nicht spielbar, nicht bezahlbar, kein gültiges Ziel).
                AiPlayDecision decision = getAi().canPlaySa(sa);
                reason = isPlayable(sa, decision) ? null : String.valueOf(decision);
            } else {
                // Wie bei Forges menschlichem Spieler: optionale Kosten, Modi, X-Wert, Ziele –
                // alles wählt der Spieler selbst.
                sa = prepareRemote(sa);
                reason = sa != null ? null : "Abgebrochen oder kein gültiges Ziel";
            }
            if (reason == null) {
                List<SpellAbility> result = new ArrayList<>();
                result.add(sa);
                return result;
            }
            JsonObject rejected = ForgeBridge.message("rejected");
            rejected.addProperty("player", seat);
            rejected.addProperty("option", choice);
            rejected.add("action", action);
            rejected.addProperty("reason", reason);
            ForgeBridge.send(rejected);
        }
    }

    /**
     * NeoForge-{@code Action} zu einer spielbaren Fähigkeit: Land → {@code PlayLand} (weitere
     * Land-Seiten derselben Karte {@code PlayLandAlt}), Zauber → {@code Cast} (weitere Zauber-
     * Varianten derselben Karte {@code CastAlt}), sonst {@code Activate} mit laufendem Index je Karte.
     */
    private static JsonObject actionJson(SpellAbility sa, Map<String, Integer> counters) {
        int card = sa.getHostCard().getId();
        String kind = sa.isLandAbility() ? "land" : sa.isSpell() ? "spell" : "ability";
        int n = counters.merge(kind + card, 1, Integer::sum) - 1;
        JsonObject a = new JsonObject();
        switch (kind) {
            case "land" -> a.addProperty("type", n == 0 ? "PlayLand" : "PlayLandAlt");
            case "spell" -> a.addProperty("type", n == 0 ? "Cast" : "CastAlt");
            default -> a.addProperty("type", "Activate");
        }
        a.addProperty("card", card);
        if (n > 0 || kind.equals("ability")) {
            a.addProperty("index", n);
        }
        return a;
    }

    private static int indexOf(JsonArray arr, JsonObject o) {
        for (int i = 0; i < arr.size(); i++) {
            if (arr.get(i).equals(o)) {
                return i;
            }
        }
        return -1;
    }

    /**
     * Bereitet das Spielen vor wie Forges Ablauf für menschliche Spieler
     * ({@code PlaySpellAbility.playSpellAbility} / {@code playAbility} im Forge-Quellcode):
     * Zusatzkosten und optionale Kosten, Modi, angesagter Typ, X-Wert, Ziele. Jede Frage geht an
     * den Spieler. Gibt die (ggf. um Zusatzkosten erweiterte) Fähigkeit zurück oder null bei
     * Abbruch / ohne gültiges Ziel. Bezahlt wird danach von Forge (vorerst automatisch).
     */
    private SpellAbility prepareRemote(SpellAbility original) {
        // Zusatzkosten (alternative) und optionale Kosten (Kicker, Verstärkung …)
        List<SpellAbility> variants = GameActionUtil.getAdditionalCostSpell(original);
        SpellAbility sa = variants.size() > 1 ? getAbilityToPlay(original.getHostCard(), variants) : original;
        if (sa == null) {
            return null;
        }
        List<OptionalCostValue> optional = GameActionUtil.getOptionalCostValues(sa);
        if (!optional.isEmpty()) {
            optional = chooseOptionalCosts(sa, optional);
        }
        sa = GameActionUtil.addOptionalCosts(sa, optional);
        sa.setActivatingPlayer(getPlayer());

        boolean needX = true;
        if (sa.getApi() == ApiType.Charm) {
            if (sa.isAnnouncing("X")) {
                needX = sa.getPayCosts().hasXInAnyCostPart();
                if (!announceValuesLikeX(sa, needX)) {
                    return null;
                }
                needX = false;
            }
            if (!CharmEffect.makeChoices(sa)) {
                return null;
            }
        }
        if (!announceType(sa) || (needX && !announceValuesLikeX(sa, true))) {
            return null;
        }
        // Ziele: setupTargets ruft für jede gezielte Teilfähigkeit chooseTargetsFor auf
        return sa.setupTargets() ? sa : null;
    }

    /** Angesagte Werte (Announce-Parameter, X-Kosten) wie im Forge-Quellcode. false = abgebrochen. */
    private boolean announceValuesLikeX(SpellAbility sa, boolean needX) {
        if (sa.isCopied() || sa.isWrapper()) {
            return true;
        }
        Card card = sa.getHostCard();
        String announce = sa.getParam("Announce");
        if (announce != null && needX) {
            for (String aVar : announce.split(",")) {
                String varName = aVar.trim();
                Range<Integer> range = AbilityUtils.getAnnouncementBounds(sa, varName);
                Integer value = announceRequirements(sa, range.getMinimum(), range.getMaximum(), varName);
                if (value == null) {
                    return false;
                }
                if ("X".equalsIgnoreCase(varName)) {
                    needX = false;
                    sa.setXManaCostPaid(value);
                } else {
                    sa.setSVar(varName, value.toString());
                    card.setSVar(varName, value.toString());
                }
            }
        }
        if (needX) {
            Cost cost = sa.getPayCosts();
            if (cost != null && cost.hasXInAnyCostPart()) {
                String sVar = sa.hasParam("XAlternative") ? sa.getParam("XAlternative") : sa.getSVar("X");
                if ("Count$xPaid".equals(sVar) || sVar.isEmpty()) {
                    Range<Integer> range = AbilityUtils.getAnnouncementBounds(sa, "X");
                    Integer value = announceRequirements(sa, range.getMinimum(), range.getMaximum(), "X");
                    if (value == null) {
                        return false;
                    }
                    sa.setXManaCostPaid(value);
                }
            } else {
                sa.setXManaCostPaid(null);
            }
        }
        return true;
    }

    /** Angesagter Typ (AnnounceType: Kreaturtyp, Zahl) wie im Forge-Quellcode. false = abgebrochen. */
    private boolean announceType(SpellAbility sa) {
        if (sa.isCopied()) {
            return true;
        }
        String announce = sa.getParam("AnnounceType");
        if (announce == null) {
            return true;
        }
        for (String aVar : announce.split(",")) {
            String varName = aVar.trim();
            if ("CreatureType".equals(varName)) {
                String choice = chooseSomeType("Creature", sa, CardType.getAllCreatureTypes(), false);
                if (choice == null) {
                    return false;
                }
                sa.getHostCard().setChosenType(choice);
            }
            if ("ChooseNumber".equals(varName)) {
                int min = Integer.parseInt(sa.getParam("Min"));
                int max = Integer.parseInt(sa.getParam("Max"));
                sa.getHostCard().setChosenNumber(chooseNumber(sa, "Zahl wählen", min, max));
            }
        }
        return true;
    }

    /**
     * Ziele für genau eine (Teil-)Fähigkeit: Liste der gültigen Ziele im Auswahlfenster (Optionen
     * tragen card_id / player). Wird von Forges {@code SpellAbility.setupTargets()} aufgerufen.
     * false = abgebrochen oder ungültig.
     */
    @Override
    public boolean chooseTargetsFor(SpellAbility cur) {
        cur.resetTargets();
        List<GameEntity> candidates = new ArrayList<>(cur.getTargetRestrictions().getAllCandidates(cur));
        int min = cur.getMinTargets();
        int max = Math.min(cur.getMaxTargets(), candidates.size());
        if (candidates.isEmpty()) {
            return min == 0;
        }
        List<String> labels = new ArrayList<>();
        for (GameEntity e : candidates) {
            labels.add(targetLabel(e));
        }
        String prompt = cur.getTargetRestrictions().getVTSelection();
        String description = "Ziel wählen" + (prompt == null || prompt.isBlank() ? "" : ": " + prompt);
        List<Integer> chosen = Decisions.targets(g(), seat, cur, description, candidates, labels, min, max);
        if (chosen == null) {
            return false;
        }
        for (int i : chosen) {
            GameEntity e = candidates.get(i);
            if (!cur.canTarget(e)) {
                return false;
            }
            cur.getTargets().add(e);
        }
        return cur.isTargetNumberValid();
    }

    /** Anzeige eines möglichen Ziels: Karte „Name (Besitzer)“ bzw. Spieler „Name“ / „Name (du)“. */
    private String targetLabel(GameEntity e) {
        if (e instanceof Card c) {
            String owner = c.getController() == getPlayer() ? "deine" : c.getController().getName();
            return c.getName() + " (" + owner + ")";
        }
        if (e instanceof Player p) {
            return p == getPlayer() ? p.getName() + " (du)" : p.getName();
        }
        return String.valueOf(e);
    }

    /** Gibt es für jede gezielte (Teil-)Fähigkeit mit Mindestzahl an Zielen mögliche Ziele? */
    private static boolean hasPossibleTargets(SpellAbility sa) {
        for (SpellAbility cur = sa; cur != null; cur = cur.getSubAbility()) {
            if (cur.usesTargeting() && cur.getMinTargets() > 0
                    && !cur.getTargetRestrictions().hasCandidates(cur)) {
                return false;
            }
        }
        return true;
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

    /** Höchstens so viele ungültige Kampf-Erklärungen, danach entscheidet die KI. */
    private static final int MAX_COMBAT_TRIES = 4;

    /**
     * {@code Decision::DeclareAttackers} (CR 508.1): Kandidaten = Kreaturen, die angreifen können,
     * Ziele = angreifbare Spieler, Planeswalker, Schlachten. Ungültige Erklärungen (z. B. eine
     * Kreatur muss angreifen) werden mit Hinweis erneut gefragt.
     */
    @Override
    public void declareAttackers(Player attacker, Combat combat) {
        List<GameEntity> defenders = new ArrayList<>(combat.getDefenders());
        List<Card> candidates = new ArrayList<>();
        for (Card c : attacker.getCreaturesInPlay()) {
            if (CombatUtil.canAttack(c) && defenders.stream().anyMatch(d -> CombatUtil.canAttack(c, d))) {
                candidates.add(c);
            }
        }
        if (candidates.isEmpty()) {
            return;
        }
        List<String> labels = new ArrayList<>();
        candidates.forEach(c -> labels.add(c.getName()));
        String error = null;
        for (int attempt = 0; attempt < MAX_COMBAT_TRIES; attempt++) {
            List<int[]> chosen = Decisions.attackers(getGame(), seat, candidates, defenders, labels, error);
            if (chosen == null) {
                break;
            }
            List<Card> added = new ArrayList<>();
            boolean ok = true;
            for (int[] pair : chosen) {
                Card c = candidates.get(pair[0]);
                GameEntity d = defenders.get(pair[1]);
                if (!CombatUtil.canAttack(c, d) || added.contains(c)) {
                    ok = false;
                    break;
                }
                combat.addAttacker(c, d);
                added.add(c);
            }
            if (ok && CombatUtil.validateAttackers(combat)) {
                return;
            }
            added.forEach(combat::removeFromCombat);
            error = "Ungültiger Angriff – bitte erneut wählen (z. B. muss eine Kreatur angreifen)";
        }
        super.declareAttackers(attacker, combat);
    }

    /**
     * {@code Decision::DeclareBlockers} (CR 509.1): Kandidaten = eigene Kreaturen, die blocken
     * können, Angreifer = alle angreifenden Kreaturen. Ungültige Blocks (Forges
     * {@code CombatUtil.validateBlocks}) werden mit Hinweis erneut gefragt.
     */
    @Override
    public void declareBlockers(Player defender, Combat combat) {
        List<Card> attackers = new ArrayList<>(combat.getAttackers());
        List<Card> candidates = new ArrayList<>();
        for (Card c : defender.getCreaturesInPlay()) {
            if (CombatUtil.canBlock(c, combat)) {
                candidates.add(c);
            }
        }
        if (candidates.isEmpty() || attackers.isEmpty()) {
            return;
        }
        List<String> labels = new ArrayList<>();
        candidates.forEach(c -> labels.add(c.getName()));
        String error = null;
        for (int attempt = 0; attempt < MAX_COMBAT_TRIES; attempt++) {
            List<int[]> chosen = Decisions.blocks(getGame(), seat, candidates, attackers, labels, error);
            if (chosen == null) {
                break;
            }
            List<int[]> added = new ArrayList<>();
            boolean ok = true;
            for (int[] pair : chosen) {
                Card b = candidates.get(pair[0]);
                Card a = attackers.get(pair[1]);
                if (!CombatUtil.canBlock(a, b, combat)) {
                    ok = false;
                    break;
                }
                combat.addBlocker(a, b);
                added.add(pair);
            }
            String problem = ok ? CombatUtil.validateBlocks(combat, defender) : "Diese Kreatur kann diesen Angreifer nicht blocken";
            if (problem == null) {
                return;
            }
            for (int[] pair : added) {
                combat.removeBlockAssignment(attackers.get(pair[1]), candidates.get(pair[0]));
            }
            error = "Ungültige Blocks: " + problem;
        }
        super.declareBlockers(defender, combat);
    }

    /** {@code Decision::Mulligan}: {@code Keep} = behalten, {@code Mulligan} = neu ziehen. */
    @Override
    public boolean mulliganKeepHand(Player firstPlayer, int cardsToReturn) {
        Boolean keep = Decisions.mulligan(getGame(), seat, cardsToReturn);
        return keep == null || keep;
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

    /** {@code Decision::ChooseModes} (CR 700.2). */
    @Override
    public List<AbilitySub> chooseModeForAbility(SpellAbility sa, List<AbilitySub> possible, int min, int num,
            boolean allowRepeat) {
        List<String> texts = new ArrayList<>();
        for (AbilitySub sub : possible) {
            texts.add(describe(sub));
        }
        List<Integer> chosen = Decisions.modes(g(), seat, sa, texts, min, num, allowRepeat, min == 0);
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
        List<Integer> chosen = Decisions.options(g(), seat, "Optionale Kosten zahlen?", sa, null,
                Decisions.opts(optionalCost), 0, optionalCost.size(), false);
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
        String r = pick(types, Decisions.nameOrType(g(), seat, "ChooseType", "Wähle: " + kindOfType, sa, null,
                types, isOptional));
        return r != null ? r : isOptional ? null : super.chooseSomeType(kindOfType, sa, validTypes, isOptional);
    }

    @Override
    public String chooseSector(Card assignee, String ai, List<String> sectors) {
        String r = pick(sectors, Decisions.option(g(), seat, "Wähle einen Sektor", null, assignee,
                Decisions.opts(sectors), false));
        return r != null ? r : super.chooseSector(assignee, ai, sectors);
    }

    @Override
    public String chooseKeywordForPump(List<String> options, SpellAbility sa, String prompt, Card tgtCard) {
        String r = pick(options, Decisions.option(g(), seat, prompt, sa, tgtCard,
                Decisions.opts(options), false));
        return r != null ? r : super.chooseKeywordForPump(options, sa, prompt, tgtCard);
    }

    @Override
    public ReplacementEffect chooseSingleReplacementEffect(List<ReplacementEffect> possible) {
        ReplacementEffect r = pick(possible, Decisions.option(g(), seat, "Welcher Ersatzeffekt gilt zuerst?",
                null, null, Decisions.opts(possible), false));
        return r != null ? r : super.chooseSingleReplacementEffect(possible);
    }

    @Override
    public StaticAbility chooseSingleStaticAbility(List<StaticAbility> possible) {
        StaticAbility r = pick(possible, Decisions.option(g(), seat, "Welche statische Fähigkeit gilt zuerst?",
                null, null, Decisions.opts(possible), false));
        return r != null ? r : super.chooseSingleStaticAbility(possible);
    }

    @Override
    public String chooseProtectionType(SpellAbility sa, List<String> choices) {
        String r = pick(choices, Decisions.option(g(), seat, "Schutz wovor?", sa, null,
                Decisions.opts(choices), false));
        return r != null ? r : super.chooseProtectionType(sa, choices);
    }

    @Override
    public CounterType chooseCounterType(List<CounterType> options, SpellAbility sa, String prompt,
            Map<String, Object> params) {
        List<Decisions.Opt> opts = new ArrayList<>();
        for (CounterType c : options) {
            opts.add(new Decisions.Opt(c.getName(), null));
        }
        CounterType r = pick(options, Decisions.option(g(), seat, prompt, sa, null, opts, false));
        return r != null ? r : super.chooseCounterType(options, sa, prompt, params);
    }

    @Override
    public ICardFace chooseSingleCardFace(SpellAbility sa, List<ICardFace> faces, String message) {
        List<Decisions.Opt> opts = new ArrayList<>();
        for (ICardFace f : faces) {
            opts.add(new Decisions.Opt(f.getName(), null));
        }
        ICardFace r = pick(faces, Decisions.option(g(), seat, message, sa, null, opts, false));
        return r != null ? r : super.chooseSingleCardFace(sa, faces, message);
    }

    @Override
    public CardState chooseSingleCardState(SpellAbility sa, List<CardState> states, String message,
            Map<String, Object> params) {
        List<Decisions.Opt> opts = new ArrayList<>();
        for (CardState st : states) {
            opts.add(new Decisions.Opt(st.getName(), null));
        }
        CardState r = pick(states, Decisions.option(g(), seat, message, sa, null, opts, false));
        return r != null ? r : super.chooseSingleCardState(sa, states, message, params);
    }

    @Override
    public Player chooseStartingPlayer(boolean isFirstGame) {
        List<Player> players = new ArrayList<>(g().getPlayers());
        List<Decisions.Opt> opts = new ArrayList<>();
        for (Player p : players) {
            opts.add(new Decisions.Opt(p == getPlayer() ? p.getName() + " (du)" : p.getName(), null));
        }
        Player r = pick(players, Decisions.option(g(), seat, "Wer beginnt?", null, null, opts, false));
        return r != null ? r : super.chooseStartingPlayer(isFirstGame);
    }

    @Override
    public Object vote(SpellAbility sa, String prompt, List<Object> options, ListMultimap<Object, Player> votes,
            Player forPlayer, boolean optional) {
        Object r = pick(options, Decisions.option(g(), seat, prompt, sa, null, Decisions.opts(options),
                optional));
        return r != null ? r : optional ? null : super.vote(sa, prompt, options, votes, forPlayer, optional);
    }

    @Override
    public Integer chooseRollToIgnore(List<Integer> rolls) {
        Integer r = pick(rolls, Decisions.option(g(), seat, "Welchen Wurf ignorieren?", null, null,
                Decisions.opts(rolls), false));
        return r != null ? r : super.chooseRollToIgnore(rolls);
    }

    @Override
    public Integer chooseRollToModify(List<Integer> rolls) {
        Integer r = pick(rolls, Decisions.option(g(), seat, "Welchen Wurf verändern?", null, null,
                Decisions.opts(rolls), true));
        return r;
    }

    @Override
    public List<Integer> chooseDiceToReroll(List<Integer> rolls) {
        List<Integer> chosen = Decisions.options(g(), seat, "Welche Würfel neu werfen?", null, null,
                Decisions.opts(rolls), 0, rolls.size(), false);
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
        Integer r = pick(sprockets, Decisions.option(g(), seat, "Wähle ein Zahnrad", null, assignee,
                Decisions.opts(sprockets), false));
        return r != null ? r : super.chooseSprocket(assignee, sprockets);
    }

    // ------------------------------------------------------------------ Ja/Nein

    @Override
    public boolean confirmAction(SpellAbility sa, PlayerActionConfirmMode mode, String message,
            List<String> options, Card cardToShow, Map<String, Object> params) {
        Boolean r = options != null && options.size() == 2
                ? Decisions.binary(g(), seat, message, sa, cardToShow, options.get(0), options.get(1))
                : Decisions.confirm(g(), seat, message, sa, cardToShow);
        return r != null ? r : super.confirmAction(sa, mode, message, options, cardToShow, params);
    }

    @Override
    public boolean confirmTrigger(WrappedAbility wrapper) {
        Boolean r = Decisions.optionalTrigger(g(), seat, wrapper.toUnsuppressedString(), wrapper,
                wrapper.getHostCard());
        return r != null ? r : super.confirmTrigger(wrapper);
    }

    @Override
    public boolean confirmReplacementEffect(ReplacementEffect replacementEffect, SpellAbility effectSA,
            GameEntity affected, String question) {
        Boolean r = Decisions.confirm(g(), seat, question, effectSA, replacementEffect.getHostCard());
        return r != null ? r : super.confirmReplacementEffect(replacementEffect, effectSA, affected, question);
    }

    @Override
    public boolean confirmStaticApplication(Card hostCard, PlayerActionConfirmMode mode, String message,
            String logic) {
        Boolean r = Decisions.confirm(g(), seat, message, null, hostCard);
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
        Boolean r = Decisions.binary(g(), seat, question, sa, null, labels[0], labels[1]);
        return r != null ? r : super.chooseBinary(sa, question, kindOfChoice, defaultVal);
    }

    @Override
    public boolean chooseFlipResult(SpellAbility sa, Player flipper, boolean call) {
        Boolean r = call
                ? Decisions.binary(g(), seat, "Kopf oder Zahl?", sa, null, "Kopf", "Zahl")
                : Decisions.binary(g(), seat, "Ergebnis des Münzwurfs wählen", sa, null, "Gewinnen", "Verlieren");
        return r != null ? r : super.chooseFlipResult(sa, flipper, call);
    }

    @Override
    public boolean willPutCardOnTop(Card c) {
        Boolean r = Decisions.binary(g(), seat, "Karte oben auf die Bibliothek legen?", null, c, "Oben",
                "Unten");
        return r != null ? r : super.willPutCardOnTop(c);
    }

    // ------------------------------------------------------------------ Zahlen

    @Override
    public int chooseNumber(SpellAbility sa, String title, int min, int max) {
        Integer r = Decisions.number(g(), seat, title, sa, null, min, max, false);
        return r != null ? r : super.chooseNumber(sa, title, min, max);
    }

    @Override
    public int chooseNumber(SpellAbility sa, String title, List<Integer> values, Player relatedPlayer) {
        Integer r = pick(values, Decisions.option(g(), seat, title, sa, null, Decisions.opts(values),
                false));
        return r != null ? r : super.chooseNumber(sa, title, values, relatedPlayer);
    }

    /** X-Kosten und andere angesagte Werte ({@code ChooseX}). Abbrechen = Zauber nicht spielen. */
    @Override
    public Integer announceRequirements(SpellAbility ability, int min, int max, String announce) {
        return Decisions.chooseX(g(), seat, ability, announce, min, max);
    }

    @Override
    public int chooseNumberForCostReduction(SpellAbility sa, int min, int max) {
        Integer r = Decisions.number(g(), seat, "Um wie viel die Kosten verringern?", sa, null, min, max, false);
        return r != null ? r : super.chooseNumberForCostReduction(sa, min, max);
    }

    @Override
    public int chooseNumberForKeywordCost(SpellAbility sa, Cost cost, KeywordInterface keyword, String prompt,
            int max) {
        Integer r = Decisions.number(g(), seat, prompt, sa, null, 0, max, false);
        return r != null ? r : super.chooseNumberForKeywordCost(sa, cost, keyword, prompt, max);
    }

    // ------------------------------------------------------------------ Farben

    private static List<Decisions.Opt> colorOptions(List<MagicColor.Color> colors) {
        List<Decisions.Opt> opts = new ArrayList<>();
        for (MagicColor.Color c : colors) {
            opts.add(new Decisions.Opt(c.getTranslatedName(), c.getShortName().toUpperCase()));
        }
        return opts;
    }

    @Override
    public byte chooseColor(String message, SpellAbility sa, ColorSet colors) {
        List<MagicColor.Color> list = new ArrayList<>(colors.getOrderedColors());
        List<String> letters = new ArrayList<>();
        for (MagicColor.Color c : list) {
            letters.add(c.getShortName().toUpperCase());
        }
        MagicColor.Color r = pick(list, Decisions.color(g(), seat, message, sa, letters));
        return r != null ? r.getColorMask() : super.chooseColor(message, sa, colors);
    }

    @Override
    public byte chooseColorAllowColorless(String message, Card c, ColorSet colors) {
        List<MagicColor.Color> list = new ArrayList<>(colors.getOrderedColors());
        List<Decisions.Opt> opts = colorOptions(list);
        opts.add(new Decisions.Opt("Farblos", "C"));
        Integer i = Decisions.option(g(), seat, message, null, c, opts, false);
        if (i == null) {
            return super.chooseColorAllowColorless(message, c, colors);
        }
        return i == list.size() ? MagicColor.COLORLESS : list.get(i).getColorMask();
    }

    @Override
    public ColorSet chooseColors(String message, SpellAbility sa, int min, int max, ColorSet options) {
        List<MagicColor.Color> list = new ArrayList<>(options.getOrderedColors());
        if (min == 1 && max == 1) {
            // genau eine Farbe: NeoForges ChooseColor
            byte c = chooseColor(message, sa, options);
            return ColorSet.fromMask(c);
        }
        List<Integer> chosen = Decisions.options(g(), seat, message, sa, null, colorOptions(list), min, max, false);
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
        List<String> names = new ArrayList<>();
        for (ICardFace f : faces) {
            names.add(f.getName());
        }
        String r = pick(names, Decisions.nameOrType(g(), seat, "NameCard", message, sa, null, names, false));
        return r != null ? r : super.chooseCardName(sa, faces, message);
    }

    /** Kartenname mit Bedingung: {@code NameCard} mit allen erlaubten Namen (sortiert, eindeutig). */
    @Override
    public String chooseCardName(SpellAbility sa, Predicate<ICardFace> cpp, String valid, String message) {
        TreeSet<String> names = new TreeSet<>();
        for (ICardFace f : StaticData.instance().getCommonCards().getAllFaces()) {
            if (cpp.test(f)) {
                names.add(f.getName());
            }
        }
        List<String> list = new ArrayList<>(names);
        String r = pick(list, Decisions.nameOrType(g(), seat, "NameCard", message, sa, null, list, false));
        return r != null ? r : super.chooseCardName(sa, cpp, valid, message);
    }
}
