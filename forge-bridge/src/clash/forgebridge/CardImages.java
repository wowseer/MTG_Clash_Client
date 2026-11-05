package clash.forgebridge;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

import forge.StaticData;
import forge.card.CardEdition;
import forge.game.card.Card;
import forge.item.IPaperCard;
import forge.item.PaperCard;
import forge.localinstance.properties.ForgeConstants;
import forge.util.ImageUtil;

/**
 * Kartenbilder (JPG) für die 3D-Karten, wie Forge und NeoForge sie benutzen: Scryfall-Bilder
 * im Bildordner von Forge ({@code pics/cards/<SET>/<Name>.fullborder.jpg}).
 *
 * <p>Gesucht wird im Bildordner dieser Forge-Installation ({@link ForgeConstants#CACHE_CARD_PICS_DIR})
 * und in dem von NeoForge (Umgebungsvariable {@code CLASH_EXTRA_PICS}, sonst
 * {@code ~/Games/NeoForge/datos/cache/pics/cards}). Fehlt ein Bild, lädt es ein Hintergrund-Thread
 * von Scryfall in den Forge-Bildordner (höchstens ~8 Anfragen je Sekunde, wie Scryfall es
 * verlangt); der nächste Zustand enthält dann den Pfad. Die Bilder gehören Wizards of the Coast
 * und kommen nie ins Git.
 */
final class CardImages {
    private CardImages() {}

    /** Bildpfad je Bild-Schlüssel ("" = keins gefunden und keins ladbar). */
    private static final Map<String, String> FOUND = new ConcurrentHashMap<>();
    /** Schlüssel, deren Download läuft oder fehlschlug (nicht erneut anstoßen). */
    private static final Set<String> REQUESTED = ConcurrentHashMap.newKeySet();
    private static final ExecutorService DOWNLOADER = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "clash-card-images");
        t.setDaemon(true);
        return t;
    });
    private static final HttpClient HTTP = HttpClient.newBuilder()
            .followRedirects(HttpClient.Redirect.NORMAL)
            .connectTimeout(Duration.ofSeconds(10))
            .build();
    /** Wartende Downloads (für die Zustandsmeldung nach dem letzten). */
    private static final java.util.concurrent.atomic.AtomicInteger QUEUED = new java.util.concurrent.atomic.AtomicInteger();
    /** Abstand zwischen zwei Scryfall-Anfragen (Millisekunden). */
    private static final long REQUEST_GAP_MS = 120;

    /**
     * Bilder einer Karte: {@code image} = ganzes Kartenbild, {@code art} = Bildausschnitt (Scryfall
     * {@code art_crop}) nur bei Karten ohne Druck mit normalem Rahmen (sonst leer), {@code crop} =
     * Bildausschnitt immer (für die Kacheln auf dem Spielfeld).
     */
    record Info(String image, String art, String crop) {
        static final Info NONE = new Info("", "", "");
    }

    /** Gewählter Anzeige-Druck je Kartenname und Druck (normaler Rahmen bevorzugt). */
    private static final Map<String, PaperCard> DISPLAY = new ConcurrentHashMap<>();

    /**
     * Bilder der angezeigten Seite von {@code c}; fehlende werden im Hintergrund geladen (dann
     * leer, ein späterer Zustand bringt den Pfad). Für die Anzeige zählt ein Druck mit normalem
     * Rahmen ({@link #regular}): ist der Druck der Karte keiner, wird der neueste normale Druck
     * gezeigt (die Karte im Spiel bleibt unverändert); gibt es keinen, kommt zusätzlich der
     * Bildausschnitt mit, damit die UI eigene Rahmen bauen kann.
     */
    static Info info(Card c) {
        if (c == null || c.isFaceDown()) {
            return Info.NONE;
        }
        IPaperCard ip = c.getPaperCard();
        // Zauber-Kopie einer vorbereiteten Kreatur (Prepare): die gedruckte Kreaturenkarte, auf der
        // der Zauber mit abgedruckt ist (wie Forges eigener Bildschlüssel für diese Seite)
        if (c.getCurrentStateName() == forge.card.CardStateName.PreparedSpell) {
            ip = forge.StaticData.instance().getCommonCards()
                    .getCard(c.getState(forge.card.CardStateName.Original).getName());
        } else if (c.isToken()) {
            String t = tokenImage(c);
            return t.isEmpty() ? Info.NONE : new Info(t, "", "");
        }
        if (!(ip instanceof PaperCard pc)) {
            return Info.NONE;
        }
        String face = c.isBackSide() || c.isFlipped() ? "back" : "";
        PaperCard shown = DISPLAY.computeIfAbsent(pc.getName() + "|" + pc.getEdition() + "|" + pc.getArtIndex(),
                k -> displayPrinting(pc));
        String image = resolve(shown, face, false);
        // Bildausschnitt: immer (Kacheln auf dem Spielfeld); `art` nur bei Sonderrahmen
        String crop = resolve(shown, face, true);
        String art = regular(shown) ? "" : crop;
        return new Info(image, art, crop);
    }

    /** Bild einer Seite (Name) der Karte {@code host}, z. B. für „Seite wählen“; sonst "". */
    static String faceImage(Card host, String faceName) {
        if (host == null || !(host.getPaperCard() instanceof PaperCard pc)) {
            return "";
        }
        PaperCard shown = DISPLAY.computeIfAbsent(pc.getName() + "|" + pc.getEdition() + "|" + pc.getArtIndex(),
                k -> displayPrinting(pc));
        String face = pc.getRules().getMainPart() != null && faceName != null
                && !faceName.equals(pc.getRules().getMainPart().getName()) ? "back" : "";
        return resolve(shown, face, false);
    }

    /**
     * Bild eines Spielsteins (Scryfall, Token-Druck der Edition): im Token-Bildordner von Forge
     * bzw. NeoForge, sonst im Hintergrund geladen; "" = (noch) keins.
     */
    private static String tokenImage(Card c) {
        if (!(c.getPaperCard() instanceof forge.item.PaperToken pt)) {
            return "";
        }
        String file;
        try {
            file = pt.getImageFilename(1);
        } catch (RuntimeException e) {
            return "";
        }
        if (file == null || file.isEmpty()) {
            return "";
        }
        String key = "token|" + file;
        String known = FOUND.get(key);
        if (known != null && !known.isEmpty()) {
            return known;
        }
        List<File> dirs = new ArrayList<>();
        dirs.add(new File(ForgeConstants.CACHE_TOKEN_PICS_DIR));
        dirs.add(new File(System.getProperty("user.home"), "Games/NeoForge/datos/cache/pics/tokens"));
        for (File d : dirs) {
            for (String ext : List.of(".jpg", ".png")) {
                File f = new File(d, file + ext);
                if (f.isFile() && f.length() > 0) {
                    FOUND.put(key, f.getAbsolutePath());
                    return f.getAbsolutePath();
                }
            }
        }
        CardEdition ed = StaticData.instance().getCardEdition(pt.getEdition());
        String cn = pt.getCollectorNumber();
        if (ed == null || cn == null || cn.isEmpty() || REQUESTED.contains(key)) {
            return "";
        }
        REQUESTED.add(key);
        String url;
        try {
            url = ForgeConstants.URL_PIC_SCRYFALL_DOWNLOAD
                    + ImageUtil.getScryfallTokenDownloadUrl(cn, ed.getTokensCode().toLowerCase(), "en", "");
        } catch (RuntimeException e) {
            return "";
        }
        File target = new File(ForgeConstants.CACHE_TOKEN_PICS_DIR, file + ".jpg");
        QUEUED.incrementAndGet();
        DOWNLOADER.submit(() -> {
            download(url, target, key);
            if (QUEUED.decrementAndGet() == 0) {
                ForgeBridge.imagesChanged();
            }
        });
        return "";
    }

    /** Editionsarten mit normalem Rahmen (Kernset, Erweiterung, Reprint-, Commander-, Draft-Sets …). */
    private static final java.util.EnumSet<CardEdition.Type> REGULAR_TYPES = java.util.EnumSet.of(
            CardEdition.Type.CORE, CardEdition.Type.EXPANSION, CardEdition.Type.REPRINT,
            CardEdition.Type.COMMANDER, CardEdition.Type.DRAFT, CardEdition.Type.STARTER,
            CardEdition.Type.DUEL_DECK);

    /**
     * Druck mit normalem Rahmen: aus einer Edition mit normalen Rahmen ({@link #REGULAR_TYPES},
     * nicht Secret Lair, Promo, Mystery Booster, „The List“ …) und im Abschnitt {@code cards} der
     * Editionsdatei (nicht Showcase, Borderless, Extended Art …). Bei solchen Drucken liegen
     * Name, Kosten und Textfeld im Bild an der gewohnten Stelle.
     */
    private static boolean regular(PaperCard pc) {
        CardEdition ed = StaticData.instance().getCardEdition(pc.getEdition());
        if (ed == null || !REGULAR_TYPES.contains(ed.getType()) || "PLST".equals(ed.getCode())) {
            return false;
        }
        String section = ed.getSectionForCollectorNumber(pc.getCollectorNumber());
        return section == null || section.isEmpty() || section.equalsIgnoreCase("cards");
    }

    /** Anzeige-Druck: der eigene, wenn er normal ist, sonst der neueste normale, sonst der eigene. */
    private static PaperCard displayPrinting(PaperCard pc) {
        if (regular(pc)) {
            return pc;
        }
        PaperCard best = null;
        java.util.Date bestDate = null;
        try {
            for (PaperCard other : StaticData.instance().getCommonCards().getAllCards(pc.getName())) {
                if (!regular(other)) {
                    continue;
                }
                CardEdition ed = StaticData.instance().getCardEdition(other.getEdition());
                java.util.Date d = ed == null ? null : ed.getDate();
                // Kernsets und Erweiterungen vor Reprint-/Commander-Sets, dann der neueste
                boolean main = ed != null && (ed.getType() == CardEdition.Type.CORE || ed.getType() == CardEdition.Type.EXPANSION);
                boolean bestMain = best != null && isMain(best);
                if (best == null || (main && !bestMain)
                        || (main == bestMain && d != null && (bestDate == null || d.after(bestDate)))) {
                    best = other;
                    bestDate = d;
                }
            }
        } catch (RuntimeException e) {
            return pc;
        }
        return best != null ? best : pc;
    }

    private static boolean isMain(PaperCard pc) {
        CardEdition ed = StaticData.instance().getCardEdition(pc.getEdition());
        return ed != null && (ed.getType() == CardEdition.Type.CORE || ed.getType() == CardEdition.Type.EXPANSION);
    }

    /** Pfad des Bilds ({@code art} = Bildausschnitt) von {@code pc}, oder "" (Download angestoßen). */
    private static String resolve(PaperCard pc, String face, boolean art) {
        String key = pc.getName() + "|" + pc.getEdition() + "|" + pc.getArtIndex() + "|" + face + (art ? "|art" : "");
        String known = FOUND.get(key);
        if (known != null && !known.isEmpty()) {
            return known;
        }
        String rel;
        try {
            rel = ImageUtil.getImageRelativePath(pc, face, true, false);
        } catch (RuntimeException e) {
            return "";
        }
        if (rel == null) {
            return "";
        }
        for (File f : candidates(rel, art)) {
            if (f.isFile() && f.length() > 0) {
                FOUND.put(key, f.getAbsolutePath());
                return f.getAbsolutePath();
            }
        }
        if (REQUESTED.add(key)) {
            File target = new File(ForgeConstants.CACHE_CARD_PICS_DIR, (art ? artcrop(rel) : fullborder(rel)) + ".jpg");
            String url;
            try {
                url = ForgeConstants.URL_PIC_SCRYFALL_DOWNLOAD
                        + ImageUtil.getScryfallDownloadUrl(pc, face.isEmpty() ? "front" : face, null, "en", art);
            } catch (RuntimeException e) {
                return "";
            }
            QUEUED.incrementAndGet();
            DOWNLOADER.submit(() -> {
                download(url, target, key);
                // nach dem letzten Bild einer Reihe einmal den neuen Zustand schicken
                if (QUEUED.decrementAndGet() == 0) {
                    ForgeBridge.imagesChanged();
                }
            });
        }
        return "";
    }

    /** „SET/Name.full“ → „SET/Name.artcrop“ (Bildausschnitt, Name wie bei NeoForge). */
    private static String artcrop(String rel) {
        return rel.endsWith(".full") ? rel.substring(0, rel.length() - ".full".length()) + ".artcrop" : rel + ".artcrop";
    }

    /** „SET/Name.full“ → „SET/Name.fullborder“ (Forges Name für Scryfall-Bilder mit Rand). */
    private static String fullborder(String rel) {
        return rel.endsWith(".full") ? rel.substring(0, rel.length() - ".full".length()) + ".fullborder" : rel;
    }

    /** Mögliche Dateien für {@code rel} in allen Bildordnern (mit und ohne Set-Ordner). */
    private static List<File> candidates(String rel, boolean art) {
        List<File> dirs = new ArrayList<>();
        dirs.add(new File(ForgeConstants.CACHE_CARD_PICS_DIR));
        String extra = System.getenv("CLASH_EXTRA_PICS");
        if (extra != null && !extra.isBlank()) {
            for (String d : extra.split(File.pathSeparator)) {
                dirs.add(new File(d));
            }
        } else {
            dirs.add(new File(System.getProperty("user.home"), "Games/NeoForge/datos/cache/pics/cards"));
        }
        String setless = rel.contains("/") ? rel.substring(rel.indexOf('/') + 1) : rel;
        List<File> out = new ArrayList<>();
        for (File d : dirs) {
            List<String> names = art ? List.of(artcrop(rel), artcrop(setless))
                    : List.of(fullborder(rel), rel, fullborder(setless), setless);
            for (String r : names) {
                out.add(new File(d, r + ".jpg"));
                out.add(new File(d, r + ".png"));
            }
        }
        return out;
    }

    private static void download(String url, File target, String key) {
        try {
            Thread.sleep(REQUEST_GAP_MS);
            HttpRequest req = HttpRequest.newBuilder(URI.create(url))
                    .timeout(Duration.ofSeconds(30))
                    .header("User-Agent", "MTGClash/0.1 (Forge bridge)")
                    .header("Accept", "image/*")
                    .GET()
                    .build();
            HttpResponse<InputStream> resp = HTTP.send(req, HttpResponse.BodyHandlers.ofInputStream());
            if (resp.statusCode() != 200) {
                resp.body().close();
                System.err.println("Kartenbild nicht ladbar (" + resp.statusCode() + "): " + url);
                return;
            }
            Files.createDirectories(target.getParentFile().toPath());
            Path tmp = Files.createTempFile(target.getParentFile().toPath(), "dl", ".part");
            try (InputStream in = resp.body()) {
                Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
            }
            Files.move(tmp, target.toPath(), StandardCopyOption.REPLACE_EXISTING);
            FOUND.put(key, target.getAbsolutePath());
        } catch (IOException e) {
            System.err.println("Kartenbild-Download fehlgeschlagen: " + url + " – " + e.getMessage());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}
