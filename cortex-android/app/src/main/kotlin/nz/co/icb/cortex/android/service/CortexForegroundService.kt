package nz.co.icb.cortex.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.os.IBinder
import android.os.Looper
import androidx.core.app.NotificationCompat
import androidx.localbroadcastmanager.content.LocalBroadcastManager
import com.google.android.gms.location.FusedLocationProviderClient
import com.google.android.gms.location.LocationCallback
import com.google.android.gms.location.LocationRequest
import com.google.android.gms.location.LocationResult
import com.google.android.gms.location.LocationServices
import com.google.android.gms.location.Priority
import com.google.gson.Gson
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import nz.co.icb.cortex.android.R
import nz.co.icb.cortex.android.db.CortexDatabase
import nz.co.icb.cortex.android.db.RecordEntity
import nz.co.icb.cortex.android.ipc.WebSocketServer

class CortexForegroundService : Service() {

    companion object {
        const val ACTION_START_SERVICE = "nz.co.icb.cortex.START_SERVICE"
        const val ACTION_STOP_SERVICE = "nz.co.icb.cortex.STOP_SERVICE"
        const val ACTION_SYNC_NOW = "nz.co.icb.cortex.SYNC_NOW"
        const val ACTION_STATUS_UPDATE = "nz.co.icb.cortex.STATUS_UPDATE"
        const val EXTRA_RUNNING = "running"
        const val EXTRA_LOG = "log"
        const val NOTIFICATION_CHANNEL_ID = "cortex_service_channel"
        const val NOTIFICATION_ID = 1001
    }

    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private lateinit var fusedLocationClient: FusedLocationProviderClient
    private lateinit var webSocketServer: WebSocketServer
    private val gson = Gson()
    private var isRunning = false

    private val locationCallback = object : LocationCallback() {
        override fun onLocationResult(result: LocationResult) {
            result.lastLocation?.let { location ->
                val data = mapOf(
                    "lat" to location.latitude,
                    "lng" to location.longitude,
                    "accuracy" to location.accuracy,
                    "timestamp" to location.time
                )
                serviceScope.launch {
                    val db = CortexDatabase.getInstance(applicationContext)
                    db.recordDao().insert(
                        RecordEntity(
                            tableName = "location",
                            data = gson.toJson(data)
                        )
                    )
                }
                broadcastLog("Location update: ${location.latitude}, ${location.longitude}")
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        fusedLocationClient = LocationServices.getFusedLocationProviderClient(this)
        createNotificationChannel()
        webSocketServer = WebSocketServer(applicationContext)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START_SERVICE, null -> startCortex()
            ACTION_STOP_SERVICE -> stopCortex()
            ACTION_SYNC_NOW -> syncNow()
        }
        return START_STICKY
    }

    private fun startCortex() {
        if (isRunning) return
        isRunning = true

        startForeground(NOTIFICATION_ID, buildNotification())
        startLocationUpdates()
        webSocketServer.start()
        broadcastStatus(true, "Cortex engine started")
    }

    private fun stopCortex() {
        isRunning = false
        stopLocationUpdates()
        try { webSocketServer.stop() } catch (_: Exception) {}
        broadcastStatus(false, "Cortex engine stopped")
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun syncNow() {
        serviceScope.launch {
            val db = CortexDatabase.getInstance(applicationContext)
            val records = db.recordDao().queryAll()
            broadcastLog("Sync: found ${records.size} records")
            // POST to configured endpoint would go here
        }
    }

    @SuppressWarnings("MissingPermission")
    private fun startLocationUpdates() {
        val request = LocationRequest.Builder(Priority.PRIORITY_BALANCED_POWER_ACCURACY, 30_000L)
            .setMinUpdateIntervalMillis(15_000L)
            .build()
        try {
            fusedLocationClient.requestLocationUpdates(request, locationCallback, Looper.getMainLooper())
        } catch (e: SecurityException) {
            broadcastLog("Location permission not granted: ${e.message}")
        }
    }

    private fun stopLocationUpdates() {
        fusedLocationClient.removeLocationUpdates(locationCallback)
    }

    private fun buildNotification(): Notification {
        return NotificationCompat.Builder(this, NOTIFICATION_CHANNEL_ID)
            .setContentTitle(getString(R.string.notification_title))
            .setContentText(getString(R.string.notification_text))
            .setSmallIcon(R.drawable.ic_cortex_notification)
            .setOngoing(true)
            .build()
    }

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            NOTIFICATION_CHANNEL_ID,
            "Cortex Service",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "Keeps Cortex scripts running in the background"
        }
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(channel)
    }

    private fun broadcastStatus(running: Boolean, log: String? = null) {
        val intent = Intent(ACTION_STATUS_UPDATE).apply {
            putExtra(EXTRA_RUNNING, running)
            log?.let { putExtra(EXTRA_LOG, it) }
        }
        LocalBroadcastManager.getInstance(this).sendBroadcast(intent)
    }

    private fun broadcastLog(message: String) {
        val intent = Intent(ACTION_STATUS_UPDATE).apply {
            putExtra(EXTRA_RUNNING, isRunning)
            putExtra(EXTRA_LOG, message)
        }
        LocalBroadcastManager.getInstance(this).sendBroadcast(intent)
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        try { webSocketServer.stop() } catch (_: Exception) {}
        stopLocationUpdates()
    }
}
