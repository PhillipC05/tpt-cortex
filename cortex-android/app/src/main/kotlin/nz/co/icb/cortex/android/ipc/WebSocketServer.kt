package nz.co.icb.cortex.android.ipc

import android.content.Context
import com.google.gson.Gson
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import nz.co.icb.cortex.android.engine.AndroidNativeRegistry
import nz.co.icb.cortex.android.engine.CortexInterpreter
import nz.co.icb.cortex.android.location.LocationRepository
import org.java_websocket.WebSocket
import org.java_websocket.handshake.ClientHandshake
import org.java_websocket.server.WebSocketServer as JWebSocketServer
import java.net.InetSocketAddress
import java.util.UUID

class WebSocketServer(private val context: Context) {

    companion object {
        const val PORT = 9911
        const val PREFS_NAME = "cortex_prefs"
        const val PREF_TOKEN = "ws_token"
    }

    private val gson = Gson()
    private var server: InternalServer? = null
    private val token: String by lazy {
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        prefs.getString(PREF_TOKEN, null) ?: run {
            val newToken = UUID.randomUUID().toString()
            prefs.edit().putString(PREF_TOKEN, newToken).apply()
            newToken
        }
    }

    fun start() {
        server?.stop()
        server = InternalServer(InetSocketAddress("127.0.0.1", PORT), token, context).also {
            it.start()
        }
    }

    fun stop() {
        server?.stop()
        server = null
    }

    inner class InternalServer(
        address: InetSocketAddress,
        private val authToken: String,
        private val ctx: Context
    ) : JWebSocketServer(address) {

        private val authenticatedConnections = mutableSetOf<WebSocket>()
        private val locationRepo = LocationRepository(ctx)
        private val nativeRegistry = AndroidNativeRegistry(ctx, locationRepo)
        private val interpreter = CortexInterpreter(nativeRegistry)

        override fun onOpen(conn: WebSocket, handshake: ClientHandshake) {
            val msg = JsonObject().apply {
                addProperty("type", "connected")
                addProperty("token", authToken)
            }
            conn.send(gson.toJson(msg))
        }

        override fun onClose(conn: WebSocket, code: Int, reason: String, remote: Boolean) {
            authenticatedConnections.remove(conn)
        }

        override fun onMessage(conn: WebSocket, message: String) {
            try {
                val json = JsonParser.parseString(message).asJsonObject
                val msgType = json.get("type")?.asString
                val providedToken = json.get("token")?.asString

                // Authenticate
                if (providedToken == authToken) {
                    authenticatedConnections.add(conn)
                }

                if (conn !in authenticatedConnections) {
                    sendError(conn, null, -32001, "Unauthorized")
                    return
                }

                // JSON-RPC dispatch
                val jsonrpc = json.get("jsonrpc")?.asString
                if (jsonrpc == "2.0") {
                    handleJsonRpc(conn, json)
                } else if (msgType != null) {
                    handleLegacyMessage(conn, json)
                }
            } catch (e: Exception) {
                sendError(conn, null, -32700, "Parse error: ${e.message}")
            }
        }

        private fun handleJsonRpc(conn: WebSocket, json: JsonObject) {
            val id = json.get("id")?.asInt
            val method = json.get("method")?.asString
            val params = json.get("params")?.asJsonObject

            when (method) {
                "ExecuteCortex" -> {
                    val script = params?.get("script")?.asString ?: ""
                    val allow = params?.get("allow")?.asString ?: "native.log,native.db,native.http,native.location"
                    val result = interpreter.execute(script, allow)

                    if (result.error != null) {
                        sendError(conn, id, -32000, result.error)
                    } else {
                        val response = JsonObject().apply {
                            addProperty("jsonrpc", "2.0")
                            id?.let { addProperty("id", it) }
                            add("result", gson.toJsonTree(mapOf("logs" to result.logs, "returnValue" to result.returnValue)))
                        }
                        conn.send(gson.toJson(response))
                    }
                }
                else -> sendError(conn, id, -32601, "Method not found: $method")
            }
        }

        private fun handleLegacyMessage(conn: WebSocket, json: JsonObject) {
            // Handle non-JSON-RPC messages for PWA compatibility
            val type = json.get("type")?.asString
            when (type) {
                "ping" -> conn.send(gson.toJson(mapOf("type" to "pong")))
                else -> sendError(conn, null, -32600, "Unknown message type: $type")
            }
        }

        private fun sendError(conn: WebSocket, id: Int?, code: Int, message: String) {
            val response = JsonObject().apply {
                addProperty("jsonrpc", "2.0")
                id?.let { addProperty("id", it) }
                add("error", gson.toJsonTree(mapOf("code" to code, "message" to message)))
            }
            conn.send(gson.toJson(response))
        }

        override fun onError(conn: WebSocket?, ex: Exception) {
            android.util.Log.e("CortexWebSocket", "WebSocket error", ex)
        }

        override fun onStart() {
            android.util.Log.i("CortexWebSocket", "WebSocket server started on port $PORT")
        }
    }
}
