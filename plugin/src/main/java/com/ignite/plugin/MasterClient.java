package com.ignite.plugin;

import org.bukkit.plugin.java.JavaPlugin;
import org.json.JSONObject;

import java.io.*;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.util.concurrent.locks.ReentrantLock;
import java.util.logging.Level;

public class MasterClient {

    private final JavaPlugin plugin;
    private final IgniteConfig config;
    private final SyncHandler syncHandler;
    private Socket socket;
    private DataInputStream in;
    private DataOutputStream out;
    private final ReentrantLock lock = new ReentrantLock();
    private volatile boolean connected = false;
    private Thread readerThread;

    public MasterClient(JavaPlugin plugin, IgniteConfig config, SyncHandler syncHandler) {
        this.plugin = plugin;
        this.config = config;
        this.syncHandler = syncHandler;
    }

    public boolean connect() {
        lock.lock();
        try {
            if (connected) {
                return true;
            }

            String host = config.getMasterHost();
            int port = config.getMasterPort();

            plugin.getLogger().info("Verbinde zum Master: " + host + ":" + port);

            socket = new Socket();
            socket.connect(new InetSocketAddress(host, port), 10000);
            socket.setKeepAlive(true);
            socket.setTcpNoDelay(true);

            out = new DataOutputStream(new BufferedOutputStream(socket.getOutputStream()));
            in = new DataInputStream(new BufferedInputStream(socket.getInputStream()));

            sendHandshake();
            startReader();

            connected = true;
            plugin.getLogger().info("Mit Master verbunden: " + host + ":" + port);
            return true;
        } catch (IOException e) {
            plugin.getLogger().log(Level.WARNING, "Verbindung zum Master fehlgeschlagen", e);
            disconnect();
            return false;
        } finally {
            lock.unlock();
        }
    }

    private void sendHandshake() throws IOException {
        JSONObject handshake = new JSONObject();
        handshake.put("Handshake", new JSONObject()
            .put("server_id", config.getServerId())
            .put("server_type", config.getServerType())
            .put("version", "1.0.0"));

        sendPacket(handshake);
    }

    private void startReader() {
        readerThread = new Thread(() -> {
            byte[] headerBuf = new byte[8];
            try {
                while (connected && !Thread.currentThread().isInterrupted()) {
                    int read = 0;
                    while (read < 8) {
                        int n = in.read(headerBuf, read, 8 - read);
                        if (n == -1) {
                            throw new EOFException("Verbindung geschlossen");
                        }
                        read += n;
                    }

                    long dataLen = bytesToLong(headerBuf);
                    if (dataLen > 100 * 1024 * 1024) {
                        plugin.getLogger().warning("Packet zu groß: " + dataLen);
                        continue;
                    }

                    byte[] data = new byte[(int) dataLen];
                    read = 0;
                    while (read < dataLen) {
                        int n = in.read(data, read, (int) (dataLen - read));
                        if (n == -1) {
                            throw new EOFException("Verbindung geschlossen");
                        }
                        read += n;
                    }

                    handlePacket(new JSONObject(new String(data)));
                }
            } catch (EOFException e) {
                plugin.getLogger().warning("Master-Verbindung geschlossen");
            } catch (IOException e) {
                plugin.getLogger().log(Level.WARNING, "Lese-Fehler vom Master", e);
            } finally {
                connected = false;
                plugin.getLogger().info("Master-Reader beendet");
            }
        }, "ignite-master-reader");
        readerThread.setDaemon(true);
        readerThread.start();
    }

    void sendPacket(JSONObject packet) throws IOException {
        lock.lock();
        try {
            if (!connected || out == null) {
                throw new IOException("Nicht verbunden");
            }
            byte[] data = packet.toString().getBytes("UTF-8");
            out.write(longToBytes(data.length));
            out.write(data);
            out.flush();
        } finally {
            lock.unlock();
        }
    }

    public void sendBackupData(String backupId, byte[] compressedData, boolean isFinal) throws IOException {
        JSONObject startPacket = new JSONObject();
        startPacket.put("BackupStart", new JSONObject()
            .put("session_id", config.getServerId() + "-" + System.currentTimeMillis())
            .put("backup_id", backupId)
            .put("total_size", compressedData.length)
            .put("chunk_count", 1));

        sendPacket(startPacket);

        String checksum = sha256Hex(compressedData);

        JSONObject chunkPacket = new JSONObject();
        chunkPacket.put("BackupChunk", new JSONObject()
            .put("session_id", config.getServerId())
            .put("backup_id", backupId)
            .put("chunk_index", 0)
            .put("data", java.util.Base64.getEncoder().encodeToString(compressedData))
            .put("checksum", checksum));

        sendPacket(chunkPacket);

        JSONObject completePacket = new JSONObject();
        completePacket.put("BackupComplete", new JSONObject()
            .put("session_id", config.getServerId())
            .put("backup_id", backupId)
            .put("checksum", checksum));

        sendPacket(completePacket);
    }

    public void sendHealthCheck(int playersOnline, long uptimeSecs) throws IOException {
        JSONObject health = new JSONObject();
        health.put("HealthCheck", new JSONObject());

        lock.lock();
        try {
            if (connected && out != null) {
                byte[] data = health.toString().getBytes("UTF-8");
                out.write(longToBytes(data.length));
                out.write(data);
                out.flush();
            }
        } finally {
            lock.unlock();
        }
    }

    private void handlePacket(JSONObject json) {
        if (json.has("HandshakeAck")) {
            JSONObject ack = json.getJSONObject("HandshakeAck");
            String sessionId = ack.getString("session_id");
            int interval = ack.optInt("backup_interval_secs", 300);
            plugin.getLogger().info("Handshake bestätigt. Session: " + sessionId +
                ", Backup-Intervall: " + interval + "s");
        } else if (json.has("BackupAck")) {
            String status = json.getJSONObject("BackupAck").getString("status");
            plugin.getLogger().info("Backup bestätigt: " + status);
        } else if (json.has("SyncCommand")) {
            JSONObject cmd = json.getJSONObject("SyncCommand");
            syncHandler.handleSyncCommand(cmd);
        } else if (json.has("SyncData")) {
            JSONObject data = json.getJSONObject("SyncData");
            syncHandler.handleSyncData(data);
        } else if (json.has("ShutdownNotice")) {
            JSONObject notice = json.getJSONObject("ShutdownNotice");
            String reason = notice.getString("reason");
            int gracePeriod = notice.getInt("grace_period_secs");
            plugin.getLogger().warning("SHUTDOWN ANGEKÜNDIGT: " + reason +
                " (Grace: " + gracePeriod + "s)");
            syncHandler.handleShutdown(reason, gracePeriod);
        } else if (json.has("Error")) {
            JSONObject err = json.getJSONObject("Error");
            plugin.getLogger().warning("Master-Fehler: " + err.optString("message", ""));
        } else {
            plugin.getLogger().warning("Unbekanntes Packet: " + json.keySet());
        }
    }

    public void disconnect() {
        connected = false;
        if (readerThread != null) {
            readerThread.interrupt();
        }
        try {
            if (socket != null) socket.close();
        } catch (IOException ignored) {}
    }

    public boolean isConnected() { return connected; }
    public void reconnect() { disconnect(); connect(); }

    private static long bytesToLong(byte[] b) {
        return ((long) b[0] & 0xFF) << 56 |
               ((long) b[1] & 0xFF) << 48 |
               ((long) b[2] & 0xFF) << 40 |
               ((long) b[3] & 0xFF) << 32 |
               ((long) b[4] & 0xFF) << 24 |
               ((long) b[5] & 0xFF) << 16 |
               ((long) b[6] & 0xFF) << 8 |
               ((long) b[7] & 0xFF);
    }

    private static byte[] longToBytes(long l) {
        return new byte[] {
            (byte) (l >> 56), (byte) (l >> 48), (byte) (l >> 40), (byte) (l >> 32),
            (byte) (l >> 24), (byte) (l >> 16), (byte) (l >> 8), (byte) l
        };
    }

    private static String sha256Hex(byte[] data) {
        try {
            java.security.MessageDigest md = java.security.MessageDigest.getInstance("SHA-256");
            byte[] hash = md.digest(data);
            StringBuilder sb = new StringBuilder();
            for (byte b : hash) {
                sb.append(String.format("%02x", b));
            }
            return sb.toString();
        } catch (Exception e) {
            return "unknown";
        }
    }
}
