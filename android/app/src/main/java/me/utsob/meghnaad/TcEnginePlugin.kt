package me.utsob.meghnaad

import com.getcapacitor.JSObject
import com.getcapacitor.Plugin
import com.getcapacitor.PluginCall
import com.getcapacitor.PluginMethod
import com.getcapacitor.annotation.CapacitorPlugin
import org.json.JSONObject

/**
 * What the page asks of the phone: the engine (the same operations the browser's engine takes), the settings routes the
 * page would ask a server for, and the connection to the bucket. The secrets of the connection are taken, never given.
 */
@CapacitorPlugin(name = "TcEngine")
class TcEnginePlugin : Plugin() {
    private fun engine() = Engines.get(context)

    private fun failed(call: PluginCall, e: Exception) = call.reject(e.message ?: e.toString())

    /** Start with these settings (JSON text). Answers whether a copy of the tasks was found on the phone. */
    @PluginMethod
    fun start(call: PluginCall) {
        try {
            val engine = engine()
            val answer = JSONObject(engine.call("config", call.getString("config") ?: "{}"))
            if (answer.has("error")) return call.reject(answer.getString("error"))
            call.resolve(JSObject().put("restored", engine.restored))
        } catch (e: Exception) {
            failed(call, e)
        }
    }

    /** `config`, `run`, `import` or `sync`; the answer is the engine's own JSON text (`sync`: whether tasks changed). */
    @PluginMethod
    fun send(call: PluginCall) {
        try {
            val engine = engine()
            val op = call.getString("op") ?: return call.reject("no operation")
            val value = when (op) {
                "sync" -> engine.sync().toString()
                "config" -> {
                    val answer = JSONObject(engine.call(op, call.getString("arg") ?: "{}"))
                    if (answer.has("error")) return call.reject(answer.getString("error"))
                    "null"
                }
                "run", "import" -> engine.call(op, call.getString("arg") ?: "")
                else -> return call.reject("unknown operation: $op")
            }
            call.resolve(JSObject().put("value", value))
        } catch (e: Exception) {
            failed(call, e)
        }
    }

    /** A request to a settings route (`/api/config`, `/api/settings`, …): `{ status, type, body }`. */
    @PluginMethod
    fun api(call: PluginCall) {
        try {
            val request = JSONObject()
                .put("method", call.getString("method") ?: "GET")
                .put("path", call.getString("path") ?: "")
                .put("body", call.getString("body") ?: "")
            val answer = JSONObject(engine().call("api", request.toString()))
            call.resolve(JSObject(answer.toString()))
        } catch (e: Exception) {
            failed(call, e)
        }
    }

    /** Forget the copy of the tasks kept on the phone. */
    @PluginMethod
    fun forget(call: PluginCall) {
        try {
            engine().forget()
            call.resolve()
        } catch (e: Exception) {
            failed(call, e)
        }
    }

    /** Whether the phone is connected, and to what. Never the secret key or the encryption secret. */
    @PluginMethod
    fun getConnection(call: PluginCall) {
        val c = engine().vault.connection()
        val out = JSObject().put("configured", c != null)
        if (c != null) {
            out.put("endpoint", c.endpoint).put("bucket", c.bucket).put("region", c.region).put("accessKey", c.accessKey)
        }
        call.resolve(out)
    }

    @PluginMethod
    fun setConnection(call: PluginCall) {
        fun field(name: String) = call.getString(name)?.trim().orEmpty()
        val c = Connection(
            field("endpoint").trimEnd('/'),
            field("bucket"),
            field("region").ifBlank { "auto" },
            field("accessKey"),
            field("secretKey"),
            field("encryptionSecret"),
        )
        if (listOf(c.endpoint, c.bucket, c.accessKey, c.secretKey, c.encryptionSecret).any { it.isBlank() }) {
            return call.reject("Every field but the region is needed.")
        }
        engine().vault.save(c)
        call.resolve()
    }

    @PluginMethod
    fun clearConnection(call: PluginCall) {
        engine().vault.clearConnection()
        call.resolve()
    }
}
