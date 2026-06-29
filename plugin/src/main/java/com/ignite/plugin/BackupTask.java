package com.ignite.plugin;

import org.bukkit.plugin.java.JavaPlugin;

import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.util.zip.GZIPOutputStream;
import java.util.logging.Level;

public class BackupTask implements Runnable {

    private final JavaPlugin plugin;
    private final MasterClient client;
    private final IgniteConfig config;
    private volatile boolean running = false;

    public BackupTask(JavaPlugin plugin, MasterClient client, IgniteConfig config) {
        this.plugin = plugin;
        this.client = client;
        this.config = config;
    }

    @Override
    public void run() {
        if (running) {
            plugin.getLogger().fine("Backup läuft bereits, übersprungen");
            return;
        }

        if (!client.isConnected()) {
            plugin.getLogger().warning("Keine Master-Verbindung, Backup übersprungen");
            return;
        }

        running = true;
        try {
            plugin.getLogger().info("Starte Server-Backup...");
            long startTime = System.currentTimeMillis();

            String backupId = config.getServerId() + "-" + startTime;

            Path serverDir = Paths.get("").toAbsolutePath();
            Path tempFile = Files.createTempFile("ignite-backup-", ".tar.gz");

            try {
                createCompressedBackup(serverDir, tempFile);
                byte[] compressedData = Files.readAllBytes(tempFile);

                if (compressedData.length > config.getMaxBackupSizeMb() * 1024L * 1024L) {
                    plugin.getLogger().warning("Backup zu groß (" +
                        compressedData.length / (1024*1024) + "MB), übersprungen");
                    return;
                }

                client.sendBackupData(backupId, compressedData, true);

                long duration = System.currentTimeMillis() - startTime;
                plugin.getLogger().info("Backup abgeschlossen: " +
                    compressedData.length / (1024*1024) + "MB in " + duration + "ms");
            } finally {
                try {
                    Files.deleteIfExists(tempFile);
                } catch (IOException ignored) {}
            }
        } catch (Exception e) {
            plugin.getLogger().log(Level.SEVERE, "Backup fehlgeschlagen", e);
        } finally {
            running = false;
        }
    }

    private void createCompressedBackup(Path sourceDir, Path outputFile) throws IOException {
        Set<String> excludePaths = new HashSet<>(Arrays.asList(config.getExcludedPaths()));

        try (OutputStream fos = Files.newOutputStream(outputFile);
             GZIPOutputStream gzos = new GZIPOutputStream(fos) {{
                 def.setLevel(config.getCompressionLevel());
             }};
             java.io.BufferedOutputStream bos = new java.io.BufferedOutputStream(gzos)) {

            // In einer Produktionsumgebung würde hier eine echte TAR-Bibliothek verwendet
            // Für die Kompatibilität: Schreibe ein einfaches TAR-Format
            writeTar(bos, sourceDir, sourceDir, excludePaths);
            bos.flush();
        }
    }

    private void writeTar(OutputStream out, Path baseDir, Path currentDir,
                          Set<String> excludePaths) throws IOException {
        try (DirectoryStream<Path> stream = Files.newDirectoryStream(currentDir)) {
            for (Path entry : stream) {
                String relativePath = baseDir.relativize(entry).toString()
                    .replace('\\', '/');

                if (shouldExclude(relativePath, excludePaths)) {
                    continue;
                }

                if (Files.isDirectory(entry)) {
                    writeTarEntry(out, relativePath + "/", 0, null);
                    writeTar(out, baseDir, entry, excludePaths);
                } else if (Files.isRegularFile(entry)) {
                    byte[] fileData = Files.readAllBytes(entry);
                    writeTarEntry(out, relativePath, fileData.length, fileData);
                }
            }
        }
    }

    private boolean shouldExclude(String path, Set<String> excludePaths) {
        for (String exclude : excludePaths) {
            String normalizedExclude = exclude.replace('\\', '/');
            if (path.equals(normalizedExclude) || path.startsWith(normalizedExclude + "/")) {
                return true;
            }
        }
        return false;
    }

    private void writeTarEntry(OutputStream out, String name, long size,
                               byte[] data) throws IOException {
        byte[] header = new byte[512];
        byte[] nameBytes = name.getBytes("UTF-8");
        System.arraycopy(nameBytes, 0, header, 0, Math.min(nameBytes.length, 100));

        // Size (octal, bytes 124-135)
        String sizeStr = Long.toOctalString(size);
        byte[] sizeBytes = sizeStr.getBytes("ASCII");
        System.arraycopy(sizeBytes, 0, header, 124, sizeBytes.length);

        // Checksum placeholder
        byte[] checksumBytes = "        ".getBytes("ASCII");
        System.arraycopy(checksumBytes, 0, header, 148, 8);

        // Type flag (0 = normal file, 5 = directory)
        header[156] = (byte) (name.endsWith("/") ? '5' : '0');

        // Calculate checksum
        int checksum = 0;
        for (byte b : header) {
            checksum += b & 0xFF;
        }
        String cs = String.format("%06o\0 ", checksum);
        byte[] csBytes = cs.getBytes("ASCII");
        System.arraycopy(csBytes, 0, header, 148, csBytes.length);

        out.write(header);

        if (data != null) {
            out.write(data);
            // Padding to 512 bytes
            int padding = (int) (512 - (size % 512));
            if (padding != 512) {
                out.write(new byte[padding]);
            }
        }
    }
}
