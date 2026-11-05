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

    /** Absoluter Pfad des Bilds der angezeigten Seite von {@code c}, oder "" (dann ggf. Download). */
    static String path(Card c) {
        if (c == null || c.isFaceDown() || c.isToken()) {
            return "";
        }
        IPaperCard ip = c.getPaperCard();
        if (!(ip instanceof PaperCard pc)) {
            return "";
        }
        String face = c.isBackSide() || c.isFlipped() ? "back" : "";
        String key = pc.getName() + "|" + pc.getEdition() + "|" + pc.getArtIndex() + "|" + face;
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
        for (File f : candidates(rel)) {
            if (f.isFile() && f.length() > 0) {
                FOUND.put(key, f.getAbsolutePath());
                return f.getAbsolutePath();
            }
        }
        if (REQUESTED.add(key)) {
            File target = new File(ForgeConstants.CACHE_CARD_PICS_DIR, fullborder(rel) + ".jpg");
            String url;
            try {
                url = ForgeConstants.URL_PIC_SCRYFALL_DOWNLOAD
                        + ImageUtil.getScryfallDownloadUrl(pc, face.isEmpty() ? "front" : face, null, "en", false);
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

    /** „SET/Name.full“ → „SET/Name.fullborder“ (Forges Name für Scryfall-Bilder mit Rand). */
    private static String fullborder(String rel) {
        return rel.endsWith(".full") ? rel.substring(0, rel.length() - ".full".length()) + ".fullborder" : rel;
    }

    /** Mögliche Dateien für {@code rel} in allen Bildordnern (mit und ohne Set-Ordner). */
    private static List<File> candidates(String rel) {
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
            for (String r : List.of(fullborder(rel), rel, fullborder(setless), setless)) {
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
