package nz.co.icb.cortex.android.engine

import android.content.Context
import android.location.Location
import com.google.gson.Gson
import kotlinx.coroutines.runBlocking
import nz.co.icb.cortex.android.db.CortexDatabase
import nz.co.icb.cortex.android.db.RecordEntity
import nz.co.icb.cortex.android.location.LocationRepository
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

class AndroidNativeRegistry(
    private val context: Context,
    private val locationRepository: LocationRepository
) {

    private val gson = Gson()
    private val logBuffer = mutableListOf<String>()

    fun dispatch(api: String, args: List<Any?>): Any? {
        return when {
            api == "native.log" -> nativeLog(args)
            api == "native.db.append" -> nativeDbAppend(args)
            api == "native.db.query" -> nativeDbQuery(args)
            api == "native.http.post" -> nativeHttpPost(args)
            api == "native.location" -> nativeLocation()
            else -> null
        }
    }

    private fun nativeLog(args: List<Any?>): String? {
        val msg = args.firstOrNull()?.toString() ?: ""
        logBuffer.add(msg)
        android.util.Log.d("CortexScript", msg)
        return msg
    }

    private fun nativeDbAppend(args: List<Any?>): Boolean {
        val table = args.getOrNull(0)?.toString() ?: return false
        val data = args.getOrNull(1)?.toString() ?: return false
        return runBlocking {
            try {
                val db = CortexDatabase.getInstance(context)
                db.recordDao().insert(RecordEntity(tableName = table, data = data))
                true
            } catch (e: Exception) {
                false
            }
        }
    }

    private fun nativeDbQuery(args: List<Any?>): String {
        val table = args.getOrNull(0)?.toString() ?: return "[]"
        return runBlocking {
            try {
                val db = CortexDatabase.getInstance(context)
                val records = db.recordDao().queryByTable(table)
                gson.toJson(records.map { it.data })
            } catch (e: Exception) {
                "[]"
            }
        }
    }

    private fun nativeHttpPost(args: List<Any?>): String {
        val urlStr = args.getOrNull(0)?.toString() ?: return ""
        val body = args.getOrNull(1)?.toString() ?: ""
        return try {
            val url = URL(urlStr)
            val conn = url.openConnection() as HttpURLConnection
            conn.requestMethod = "POST"
            conn.setRequestProperty("Content-Type", "application/json")
            conn.doOutput = true
            conn.connectTimeout = 10_000
            conn.readTimeout = 10_000
            OutputStreamWriter(conn.outputStream).use { it.write(body) }
            conn.inputStream.bufferedReader().use { it.readText() }
        } catch (e: Exception) {
            "{\"error\": \"${e.message}\"}"
        }
    }

    private fun nativeLocation(): String {
        val location: Location? = locationRepository.lastLocation
        return if (location != null) {
            gson.toJson(mapOf(
                "lat" to location.latitude,
                "lng" to location.longitude,
                "accuracy" to location.accuracy,
                "timestamp" to location.time
            ))
        } else {
            "{\"error\": \"location not available\"}"
        }
    }

    fun getLogBuffer(): List<String> = logBuffer.toList()
    fun clearLogBuffer() = logBuffer.clear()
}
