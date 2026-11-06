/**
 * Öffentliche Schnittstelle der Forge-Brücke von Neo Forge 3D: {@link clash.forgebridge.ClashBridge}
 * (Einbetten, ferngesteuerter Sitz, eigenständiger Betrieb) und
 * {@link clash.forgebridge.TableLauncher} (Tisch-Prozess starten und per lokalem Socket
 * verbinden). Nur diese beiden Klassen sind für Gastgeber gedacht; {@code engine} ist intern,
 * {@code app} ist der Programmstart des eigenständigen Prozesses.
 */
package clash.forgebridge;
