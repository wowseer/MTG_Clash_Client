/**
 * Intern: Anbindung an die Forge-Engine. {@code RemotePlayerController} beantwortet Forges
 * Fragen an einen Sitz über das Protokoll ({@code Decisions} baut die {@code Decision}-Nachrichten
 * und liest die {@code Choice}-Antworten), {@code StateJson} schreibt den Spielzustand,
 * {@code EventForwarder}/{@code LogForwarder} Ereignisse und Spielprotokoll, {@code BridgeCore}
 * verwaltet Verbindung, Anfragen und Partie. Gastgeber benutzen stattdessen
 * {@link clash.forgebridge.ClashBridge}.
 */
package clash.forgebridge.engine;
