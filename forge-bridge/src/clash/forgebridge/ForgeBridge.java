package clash.forgebridge;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import forge.GuiDesktop;
import forge.LobbyPlayer;
import forge.deck.Deck;
import forge.deck.io.DeckSerializer;
import forge.game.Game;
import forge.game.GameOutcome;
import forge.game.GameRules;
import forge.game.GameType;
import forge.game.Match;
import forge.game.player.RegisteredPlayer;
import forge.gui.GuiBase;
import forge.model.FModel;
import forge.player.GamePlayerUtil;
import forge.util.MyRandom;

/**
 * Brücke zwischen MTG Clash (Godot/Rust) und der Forge-Regel-Engine.
 *
 * <p>Läuft als eigener Prozess. Protokoll: JSON Lines (UTF-8), eine Nachricht pro Zeile.
 * Befehle kommen über stdin, Antworten/Ereignisse gehen über stdout. Alle sonstigen Ausgaben
 * von Forge werden nach stderr umgeleitet, damit stdout sauber bleibt.
 * Das vollständige Protokoll steht in {@code docs/Forge-Schnittstelle.md}.
 *
 * <p>Muss mit dem Forge-Installationsordner als Arbeitsverzeichnis gestartet werden
 * (Forge lädt {@code res/} relativ dazu).
 */
public final class ForgeBridge {
    /** Version des Protokolls; wird in "ready" gemeldet. */
    public static final int PROTOCOL_VERSION = 3;

    private static PrintStream out;
    private static final Object OUT_LOCK = new Object();
    private static final Map<Integer, CompletableFuture<JsonElement>> PENDING = new ConcurrentHashMap<>();
    private static final AtomicInteger NEXT_ID = new AtomicInteger(1);
    private static volatile Thread gameThread;
    private static volatile Game currentGame;

    private ForgeBridge() {}

    public static void main(String[] args) throws Exception {
        out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
        System.setOut(System.err);
        System.setProperty("java.awt.headless", "true");

        try {
            GuiBase.setInterface(new GuiDesktop());
            FModel.initialize(null, null);
        } catch (Throwable t) {
            sendError("init_failed", t);
            System.exit(1);
        }

        JsonObject ready = message("ready");
        ready.addProperty("protocol", PROTOCOL_VERSION);
        ready.addProperty("working_dir", new File(".").getAbsolutePath());
        send(ready);

        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        String line;
        while ((line = in.readLine()) != null) {
            line = line.trim();
            if (line.isEmpty()) {
                continue;
            }
            try {
                handle(JsonParser.parseString(line).getAsJsonObject());
            } catch (Exception e) {
                sendError("bad_command", e);
            }
        }
        // stdin geschlossen: Godot ist weg -> beenden.
        System.exit(0);
    }

    // ------------------------------------------------------------------ Befehle

    private static void handle(JsonObject cmd) {
        String name = cmd.has("cmd") ? cmd.get("cmd").getAsString() : "";
        switch (name) {
            case "ping" -> {
                JsonObject pong = message("pong");
                if (cmd.has("nonce")) {
                    pong.add("nonce", cmd.get("nonce"));
                }
                send(pong);
            }
            case "start_match" -> startMatch(cmd);
            case "answer" -> {
                int id = cmd.get("id").getAsInt();
                CompletableFuture<JsonElement> f = PENDING.remove(id);
                if (f == null) {
                    sendError("unknown_request", "Keine offene Anfrage mit id " + id);
                } else {
                    f.complete(cmd.has("choice") ? cmd.get("choice") : JsonNull.INSTANCE);
                }
            }
            case "state" -> {
                Game g = currentGame;
                if (g == null) {
                    sendError("no_match", "Kein laufendes Spiel");
                } else {
                    JsonObject st = message("state");
                    st.add("state", StateJson.snapshot(g, cmd.has("viewer") ? cmd.get("viewer").getAsInt() : -1));
                    send(st);
                }
            }
            case "quit" -> System.exit(0);
            default -> sendError("unknown_command", "Unbekannter Befehl: " + name);
        }
    }

    private static void startMatch(JsonObject cmd) {
        if (gameThread != null && gameThread.isAlive()) {
            sendError("match_running", "Es läuft bereits ein Spiel");
            return;
        }
        JsonArray decks = cmd.getAsJsonArray("decks");
        JsonArray remote = cmd.has("remote") ? cmd.getAsJsonArray("remote") : new JsonArray();
        JsonArray names = cmd.has("names") ? cmd.getAsJsonArray("names") : new JsonArray();
        if (decks == null || decks.size() < 2) {
            sendError("bad_command", "start_match braucht mindestens zwei Decks");
            return;
        }
        if (cmd.has("seed")) {
            MyRandom.setRandom(new Random(cmd.get("seed").getAsLong()));
        }

        List<RegisteredPlayer> players = new ArrayList<>();
        for (int i = 0; i < decks.size(); i++) {
            Deck deck = loadDeck(decks.get(i).getAsString());
            if (deck == null) {
                sendError("deck_not_found", "Deck nicht gefunden: " + decks.get(i).getAsString());
                return;
            }
            String playerName = i < names.size() ? names.get(i).getAsString() : "Spieler " + (i + 1);
            boolean isRemote = i < remote.size() && remote.get(i).getAsBoolean();
            LobbyPlayer lobby = isRemote
                    ? new RemoteLobbyPlayer(playerName, i)
                    : GamePlayerUtil.createAiPlayer(playerName, 0);
            RegisteredPlayer rp = new RegisteredPlayer(deck);
            rp.setPlayer(lobby);
            if (cmd.has("starting_life")) {
                rp.setStartingLife(cmd.get("starting_life").getAsInt());
            }
            players.add(rp);
        }

        GameRules rules = new GameRules(GameType.Constructed);
        rules.setGamesPerMatch(1);
        Match match = new Match(rules, players, "MTG Clash");
        Game game = match.createGame();
        currentGame = game;
        game.subscribeToEvents(new EventForwarder(game));
        game.getGameLog().addObserver(new LogForwarder(game));

        gameThread = new Thread(() -> runGame(match, game), "forge-game");
        gameThread.setDaemon(true);
        gameThread.start();
    }

    private static void runGame(Match match, Game game) {
        JsonObject started = message("match_started");
        started.add("state", StateJson.snapshot(game, -1));
        send(started);
        try {
            match.startGame(game);
        } catch (Throwable t) {
            sendError("game_crashed", t);
        }
        JsonObject over = message("game_over");
        GameOutcome outcome = game.getOutcome();
        if (outcome != null) {
            over.addProperty("draw", outcome.isDraw());
            LobbyPlayer winner = outcome.getWinningLobbyPlayer();
            over.addProperty("winner", winner == null ? null : winner.getName());
            over.addProperty("reason", String.valueOf(outcome.getWinCondition()));
        }
        send(over);
        // currentGame bleibt gesetzt, damit "state" auch nach Spielende den Endstand liefert.
        // Offene Anfragen beenden, damit kein Thread hängen bleibt.
        PENDING.values().forEach(f -> f.complete(JsonNull.INSTANCE));
        PENDING.clear();
    }

    /** Lädt ein Deck aus einer .dck-Datei oder (falls keine Datei) aus Forges Constructed-Decks. */
    private static Deck loadDeck(String spec) {
        File file = new File(spec);
        if (file.isFile()) {
            return DeckSerializer.fromFile(file);
        }
        return FModel.getDecks().getConstructed().get(spec);
    }

    // ------------------------------------------------------------------ Ausgabe

    /** Neue Nachricht mit gesetztem "type". */
    static JsonObject message(String type) {
        JsonObject o = new JsonObject();
        o.addProperty("type", type);
        return o;
    }

    /** Schreibt eine Nachricht als eine JSON-Zeile auf stdout (thread-sicher). */
    static void send(JsonObject o) {
        synchronized (OUT_LOCK) {
            out.println(o.toString());
            out.flush();
        }
    }

    static void sendError(String code, Object detail) {
        JsonObject e = message("error");
        e.addProperty("code", code);
        if (detail instanceof Throwable t) {
            e.addProperty("message", t.toString());
            t.printStackTrace(System.err);
        } else {
            e.addProperty("message", String.valueOf(detail));
        }
        send(e);
    }

    /**
     * Stellt dem Client eine Entscheidung (Protokoll v3) und blockiert den Spiel-Thread bis zur
     * Antwort ({"cmd":"answer","id":…,"choice":{…}}). {@code envelope} enthält
     * {@code decision} (NeoForge-{@code Decision}) und die Anzeigehilfen (siehe {@link Decisions});
     * {@code type}, {@code id} und {@code player} werden hier ergänzt. Liefert die {@code choice}
     * (oder JsonNull).
     */
    static JsonElement request(int player, JsonObject envelope) {
        int id = NEXT_ID.getAndIncrement();
        CompletableFuture<JsonElement> f = new CompletableFuture<>();
        PENDING.put(id, f);
        JsonObject req = message("request");
        req.addProperty("id", id);
        req.addProperty("player", player);
        for (Map.Entry<String, JsonElement> e : envelope.entrySet()) {
            req.add(e.getKey(), e.getValue());
        }
        send(req);
        try {
            return f.get();
        } catch (Exception e) {
            return JsonNull.INSTANCE;
        }
    }
}
