package com.ignite.plugin;

import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.plugin.java.JavaPlugin;

public class IgniteCommand implements CommandExecutor {

    private final JavaPlugin plugin;

    public IgniteCommand(JavaPlugin plugin) {
        this.plugin = plugin;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length == 0) {
            sender.sendMessage("§6=== Ignite Plugin ===");
            sender.sendMessage("§7/ignite reload  - Config neuladen");
            sender.sendMessage("§7/ignite status  - Status anzeigen");
            sender.sendMessage("§7/ignite backup  - Manuelles Backup auslösen");
            return true;
        }

        switch (args[0].toLowerCase()) {
            case "reload":
                IgniteConfig config = ((IgnitePlugin) plugin).getIgniteConfig();
                config.reload();
                sender.sendMessage("§aConfig neu geladen.");
                break;

            case "status":
                MasterClient client = ((IgnitePlugin) plugin).getMasterClient();
                sender.sendMessage("§6Master: " + (client.isConnected() ? "§aVerbunden" : "§cGetrennt"));
                sender.sendMessage("§6Server-ID: " + plugin.getConfig().getString("server.id"));
                sender.sendMessage("§6Backup-Intervall: " + plugin.getConfig().getInt("backup.interval-seconds") + "s");
                break;

            case "backup":
                sender.sendMessage("§eStarte manuelles Backup...");
                ((IgnitePlugin) plugin).triggerBackup();
                sender.sendMessage("§aBackup gestartet.");
                break;

            default:
                sender.sendMessage("§cUnbekannter Befehl: " + args[0]);
        }

        return true;
    }
}
