package com.ignite.plugin;

import org.bukkit.configuration.file.FileConfiguration;
import org.bukkit.plugin.java.JavaPlugin;

public class IgniteConfig {

    private final JavaPlugin plugin;
    private FileConfiguration config;

    public IgniteConfig(JavaPlugin plugin) {
        this.plugin = plugin;
        this.config = plugin.getConfig();
        config.options().copyDefaults(true);
        plugin.saveConfig();
    }

    public void reload() {
        plugin.reloadConfig();
        this.config = plugin.getConfig();
    }

    public String getMasterHost() {
        return config.getString("master.host", "127.0.0.1");
    }

    public int getMasterPort() {
        return config.getInt("master.port", 9100);
    }

    public int getBackupIntervalSeconds() {
        return config.getInt("backup.interval-seconds", 300);
    }

    public int getCompressionLevel() {
        return config.getInt("backup.compression-level", 6);
    }

    public String getServerId() {
        return config.getString("server.id", "server_a");
    }

    public String getServerType() {
        return config.getString("server.type", "server_a");
    }

    public int getMaxBackupSizeMb() {
        return config.getInt("backup.max-size-mb", 5000);
    }

    public String[] getExcludedPaths() {
        return config.getStringList("backup.exclude-paths")
                .toArray(new String[0]);
    }

    static void writeDefaults(FileConfiguration config) {
        config.addDefault("master.host", "127.0.0.1");
        config.addDefault("master.port", 9100);
        config.addDefault("server.id", "server_a");
        config.addDefault("server.type", "server_a");
        config.addDefault("backup.interval-seconds", 300);
        config.addDefault("backup.compression-level", 6);
        config.addDefault("backup.max-size-mb", 5000);
        config.addDefault("backup.exclude-paths", java.util.Arrays.asList(
            "cache", "logs", "crash-reports", "plugins/IgnitePlugin"
        ));
    }
}
