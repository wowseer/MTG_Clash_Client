package clash.forgebridge.app;

import clash.forgebridge.ClashBridge;
import forge.GuiDesktop;
import forge.gui.GuiBase;
import forge.model.FModel;

/**
 * Einstieg der eigenständigen Brücke (eigener Prozess, gestartet von Godot): initialisiert Forge
 * mit der Desktop-Oberflächen-Schnittstelle und startet {@link ClashBridge#runStandalone}.
 *
 * <p>Nicht Teil der Einbettung in NeoForge (dort ist Forge schon geladen und es gibt kein
 * {@code forge-gui-desktop}).
 */
public final class BridgeMain {
    private BridgeMain() {}

    public static void main(String[] args) throws Exception {
        ClashBridge.prepareStandalone();
        System.setProperty("java.awt.headless", "true");
        try {
            GuiBase.setInterface(new GuiDesktop());
            FModel.initialize(null, null);
        } catch (Throwable t) {
            ClashBridge.sendError("init_failed", t);
            System.exit(1);
        }
        ClashBridge.runStandalone();
    }
}
