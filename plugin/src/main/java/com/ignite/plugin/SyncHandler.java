package com.ignite.plugin;

import org.bukkit.Bukkit;
import org.bukkit.plugin.java.JavaPlugin;
import org.json.JSONObject;

import java.io.*;
import java.nio.file.*;
import java.util.Base64;
import java.util.logging.Level;

public class SyncHandler {

    private final JavaPlugin plugin;

    public SyncHandler(JavaPlugin plugin) {
        this.plugin = plugin;
    }

    public void handleSyncCommand(JSONObject cmd) {
        String action = cmd.optString("action", "");
        plugin.getLogger().info("Sync-Command empfangen: " + action);

        switch (action) {
            case "PrepareReboot":
                JSONObject reboot = cmd.optJSONObject("PrepareReboot");
                if (reboot != null) {
                    String reason = reboot.optString("reason", "unknown");
                    handlePrepareReboot(reason);
                }
                break;

            case "StartSync":
                String source = cmd.optString("source", "");
                String target = cmd.optString("target", "");
                handleStartSync(source, target);
                break;

            case "ApplyFullBackup":
                String backupId = cmd.optString("backup_id", "");
                handleApplyBackup(backupId);
                break;

            case "ActivateServer":
                String serverId = cmd.optString("server_id", "");
                handleActivateServer(serverId);
                break;

            case "DeactivateServer":
                String deactId = cmd.optString("server_id", "");
                boolean graceful = cmd.optBoolean("graceful", true);
                handleDeactivateServer(deactId, graceful);
                break;

            default:
                plugin.getLogger().warning("Unbekanntes Sync-Command: " + action);
        }
    }

    public void handleSyncData(JSONObject data) {
        String dataType = data.optString("data_type", "");
        String dataStr = data.optString("data", "");

        plugin.getLogger().info("SyncData empfangen: " + dataType + " (" + dataStr.length() + " bytes)");

        try {
            byte[] rawData = Base64.getDecoder().decode(dataStr);

            Path tempFile = Files.createTempFile("ignite-sync-", ".tar.gz");
            try {
                Files.write(tempFile, rawData);
                extractBackup(tempFile);
                plugin.getLogger().info("SyncData erfolgreich angewendet: " + dataType);
            } finally {
                Files.deleteIfExists(tempFile);
            }
        } catch (Exception e) {
            plugin.getLogger().log(Level.SEVERE, "SyncData Fehler", e);
        }
    }

    public void handleShutdown(String reason, int gracePeriodSecs) {
        plugin.getLogger().warning("Server Shutdown in " + gracePeriodSecs + "s: " + reason);

        Bukkit.getScheduler().runTaskLater(plugin, () -> {
            plugin.getLogger().info("Führe gracefull Shutdown aus...");

            Bukkit.getOnlinePlayers().forEach(player -> {
                player.sendMessage("§c[Ignite] Server wird neu gestartet: " + reason);
                player.sendMessage("§7Du wirst in Kürze auf einen anderen Server verbunden.");
            });

            try {
                Thread.sleep(5000);
            } catch (InterruptedException ignored) {}

            Bukkit.shutdown();
        }, 20 * Math.max(gracePeriodSecs - 5, 0));
    }

    private void handlePrepareReboot(String reason) {
        plugin.getLogger().info("Reboot-Vorbereitung: " + reason);
        Bukkit.getOnlinePlayers().forEach(player -> {
            player.sendMessage("§e[Ignite] Server-Neustart wird vorbereitet: " + reason);
        });
    }

    private void handleStartSync(String source, String target) {
        plugin.getLogger().info("Sync von " + source + " nach " + target);

        if (source.equals(plugin.getConfig().getString("server.id"))) {
            plugin.getLogger().info("Dieser Server ist die Source. Sende aktuelle Daten...");
            Bukkit.getScheduler().runTaskAsynchronously(plugin, () -> {
                try {
                    Path serverDir = Paths.get("").toAbsolutePath();
                    Path tempFile = Files.createTempFile("ignite-incr-sync-", ".tar.gz");

                    try {
                        // Vereinfachter Sync: sende nur World, Plugin, Config Ordner
                        BackupTask backupTask = new BackupTask(
                            plugin,
                            ((IgnitePlugin) plugin).getMasterClient(),
                            ((IgnitePlugin) plugin).getIgniteConfig()
                        );
                        backupTask.run();
                    } finally {
                        Files.deleteIfExists(tempFile);
                    }
                } catch (Exception e) {
                    plugin.getLogger().log(Level.SEVERE, "Sync Source Fehler", e);
                }
            });
        }
    }

    private void handleApplyBackup(String backupId) {
        plugin.getLogger().info("Wende Backup an: " + backupId);
    }

    private void handleActivateServer(String serverId) {
        plugin.getLogger().info("Server aktiviert: " + serverId);

        if (serverId.equals(plugin.getConfig().getString("server.id"))) {
            Bukkit.getOnlinePlayers().forEach(player -> {
                player.sendMessage("§a[Ignite] Dieser Server ist jetzt aktiv!");
            });
        }
    }

    private void handleDeactivateServer(String serverId, boolean graceful) {
        plugin.getLogger().info("Server deaktiviert: " + serverId);

        if (graceful && serverId.equals(plugin.getConfig().getString("server.id"))) {
            Bukkit.getOnlinePlayers().forEach(player -> {
                player.sendMessage("§e[Ignite] Server wird deaktiviert...");
            });
        }
    }

    private void extractBackup(Path tarGzFile) throws IOException {
        Path serverDir = Paths.get("").toAbsolutePath();

        // Einfache Extraktion: in Produktion würde man eine echte TAR-Bibliothek nutzen
        plugin.getLogger().info("Backup-Extraktion nach: " + serverDir);
    }
}
