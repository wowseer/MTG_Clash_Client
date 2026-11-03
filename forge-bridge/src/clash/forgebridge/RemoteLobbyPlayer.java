package clash.forgebridge;

import com.google.gson.JsonObject;

import forge.LobbyPlayer;
import forge.ai.PlayerControllerAi;
import forge.game.Game;
import forge.game.player.IGameEntitiesFactory;
import forge.game.player.Player;
import forge.game.player.PlayerController;

/**
 * Lobby-Spieler, der von MTG Clash (über die Brücke) gesteuert wird.
 * Erzeugt im Spiel einen {@link Player} mit {@link RemotePlayerController}.
 */
public class RemoteLobbyPlayer extends LobbyPlayer implements IGameEntitiesFactory {
    /** Sitzplatz-Index (Reihenfolge der Decks in "start_match"). */
    private final int seat;

    public RemoteLobbyPlayer(String name, int seat) {
        super(name);
        this.seat = seat;
    }

    public int getSeat() {
        return seat;
    }

    @Override
    public Player createIngamePlayer(Game game, int id) {
        Player player = new Player(getName(), game, id);
        player.setFirstController(new RemotePlayerController(game, player, this));
        return player;
    }

    @Override
    public PlayerController createMindSlaveController(Player master, Player slave) {
        // Fremdsteuerung (z. B. "Gedankenkontrolle"): vorerst von der Forge-KI übernommen.
        return new PlayerControllerAi(slave.getGame(), slave, this);
    }

    @Override
    public void hear(LobbyPlayer player, String message) {
        JsonObject m = ForgeBridge.message("chat");
        m.addProperty("from", player == null ? null : player.getName());
        m.addProperty("text", message);
        ForgeBridge.send(m);
    }
}
