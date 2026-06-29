package com.ignite.plugin;

import org.bukkit.plugin.java.JavaPlugin;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.logging.Level;

public class IgnitePlugin extends JavaPlugin {

    private MasterClient masterClient;
    private BackupTask backupTask;
    private SyncHandler syncHandler;
    private ScheduledExecutorService executor;
    private IgniteConfig config;

    @Override
    public void onEnable() {
        saveDefaultConfig();
        IgniteConfig.writeDefaults(getConfig());
        saveConfig();
        this.config = new IgniteConfig(this);

        this.executor = Executors.newScheduledThreadPool(4);
        this.syncHandler = new SyncHandler(this);
        this.masterClient = new MasterClient(this, config, syncHandler);

        getCommand("ignite").setExecutor(new IgniteCommand(this));

        if (!masterClient.connect()) {
            getLogger().warning("Konnte nicht zum Master verbinden. Versuche im Hintergrund...");
            executor.scheduleWithFixedDelay(() -> {
                if (!masterClient.isConnected()) {
                    masterClient.connect();
                }
            }, 15, 30, TimeUnit.SECONDS);
        }

        int interval = config.getBackupIntervalSeconds();
        this.backupTask = new BackupTask(this, masterClient, config);
        executor.scheduleAtFixedRate(backupTask, interval, interval, TimeUnit.SECONDS);

        getLogger().info("=== Ignite Plugin aktiviert ===");
        getLogger().info("Master: " + config.getMasterHost() + ":" + config.getMasterPort());
        getLogger().info("Backup-Intervall: " + interval + "s");
        getLogger().info("Server-ID: " + config.getServerId());
    }

    @Override
    public void onDisable() {
        if (masterClient != null) {
            masterClient.disconnect();
        }
        if (executor != null) {
            executor.shutdownNow();
        }
        getLogger().info("Ignite Plugin deaktiviert");
    }

    public void triggerBackup() {
        executor.submit(backupTask);
    }

    public MasterClient getMasterClient() { return masterClient; }
    public SyncHandler getSyncHandler() { return syncHandler; }
    public IgniteConfig getIgniteConfig() { return config; }
}
