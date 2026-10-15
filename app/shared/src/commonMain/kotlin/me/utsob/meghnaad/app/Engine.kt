package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/** What a sync found: whether the tasks changed, and which of the shared files came from the bucket. */
class SyncResult(val changed: Boolean, val settings: List<String> = emptyList())

/** What the message of a sync starts with when the tasks were synced somewhere else before (see [EnginePort.sync]). */
const val BACKEND_CHANGED = "backend-changed"

/** A sync that did not work (offline, the bucket unreachable, a wrong key): the reason, in words to show. */
class SyncFailed(message: String) : Exception(message)

/** Where the engine is: native on every target (Rust, in the app's process). Every call answers in JSON text. */
interface EnginePort {
    /** `config`, `run`, `import` or `api`, with its argument as text. */
    suspend fun call(op: String, arg: String): String

    /** Whether the tasks were found on the device when the engine started, so a sync is not needed to have any. */
    val restored: Boolean

    /**
     * Pull and push with the bucket or the sync server, and keep the shared files in step when asked. A device that is not
     * connected has nothing to sync with, which is a way of using it, not a failure. Throws [SyncFailed]; when the tasks were
     * last synced somewhere else, its message starts with [BACKEND_CHANGED] and nothing has changed: syncing again with
     * [switch] (`merge` keeps this device's tasks and adds them to the new place, `replace` starts from what it holds) goes on.
     */
    suspend fun sync(switch: String? = null): SyncResult

    /** Throw away the copy kept on the device; the next start fetches the tasks again from the bucket. */
    suspend fun forget()
}

/** What the page and the clock are, for a request: the offset east of UTC in seconds, and the time in seconds since 1970. */
interface Clock {
    fun utcOffsetSeconds(): Int

    /** The offset east of UTC in force at [epochSeconds] (it differs from today's across a change of summer time). */
    fun offsetAt(epochSeconds: Long): Int

    fun nowSeconds(): Double
}

/** Answers already given to the engine's questions about one command. */
data class Answers(
    /** Yes to the plain questions (undo, a command with no filter). */
    val confirmed: Boolean = false,
    /** The tasks to go ahead with, when asked which ones. */
    val approved: List<String>? = null,
    /** The follow-up questions answered yes. */
    val extras: List<String>? = null,
)

/**
 * The engine as the screens use it: the same operations as `web/src/lib/api.ts`, with the settings routes answered by the
 * engine itself (`api`), as on the phone today.
 */
class Tw(private val port: EnginePort, private val clock: Clock) {
    private suspend fun api(method: String, path: String, body: String = ""): JsonObject {
        val request = buildJsonObject {
            put("method", method)
            put("path", path)
            put("body", body)
        }
        val answer = engineJson.parseToJsonElement(port.call("api", request.toString())).jsonObject
        answer["error"]?.let { throw EngineError(it.jsonPrimitive.content) }
        return answer
    }

    /** Read the settings and give them to the engine: it cannot run a command before this. */
    suspend fun start(): ConfigResponse {
        val answer = api("GET", "/api/config")
        val status = answer["status"]?.jsonPrimitive?.content?.toIntOrNull() ?: 200
        val body = engineJson.parseToJsonElement(answer.getValue("body").jsonPrimitive.content).jsonObject
        if (status !in 200..299) throw EngineError(body["error"]?.jsonPrimitive?.content ?: "settings: status $status", status)
        val config = body.getValue("config")
        val started = engineJson.parseToJsonElement(port.call("config", config.toString())).jsonObject
        started["error"]?.let { throw EngineError(it.jsonPrimitive.content) }
        return engineJson.decodeFromJsonElement(ConfigResponse.serializer(), body)
    }

    /** Run a command line as typed, or pre-split arguments. */
    suspend fun run(line: String? = null, args: List<String>? = null, answers: Answers = Answers()): CliResponse {
        val request = buildJsonObject {
            if (line != null) put("line", line)
            if (args != null) put("args", buildJsonArray { args.forEach { add(JsonPrimitive(it)) } })
            put("tz", clock.utcOffsetSeconds())
            put("now", clock.nowSeconds())
            if (answers.confirmed) put("confirmed", true)
            answers.approved?.let { put("approved", buildJsonArray { it.forEach { k -> add(JsonPrimitive(k)) } }) }
            answers.extras?.let { put("extras", buildJsonArray { it.forEach { k -> add(JsonPrimitive(k)) } }) }
        }
        val got = parseResponse(port.call("run", request.toString()))
        // `config` changed the settings: they are kept (where the device keeps them), and the engine takes them up.
        val config = got.config?.takeIf { it !is kotlinx.serialization.json.JsonNull } ?: return got
        return try {
            val saved = api("PUT", "/api/config/engine", config.toString())
            if ((saved["status"]?.jsonPrimitive?.content?.toIntOrNull() ?: 200) !in 200..299) error("not saved")
            engineJson.parseToJsonElement(port.call("config", config.toString())).jsonObject["error"]?.let { error(it.jsonPrimitive.content) }
            CliResponse(got.wrote, got.feedback, got.result, got.command, null)
        } catch (e: Exception) {
            CliResponse(got.wrote, got.feedback, CliResult.Error(ErrorResult("The settings could not be saved. Nothing was changed.")), got.command, null)
        }
    }

    /** One of the settings routes (`/api/settings`, `/api/hooks`, …) answered by the engine: status, and the body as text. */
    suspend fun request(method: String, path: String, body: String = ""): Pair<Int, String> {
        val answer = api(method, path, body)
        return (answer["status"]?.jsonPrimitive?.content?.toIntOrNull() ?: 200) to (answer["body"]?.jsonPrimitive?.content ?: "")
    }

    /** What the settings say: read again (a setting changed), and given to the engine. */
    suspend fun config(): ConfigResponse = start()

    /** An import of a file of tasks (what `export` writes): without [apply] it only reports what would happen. */
    suspend fun import(text: String, apply: Boolean): JsonObject {
        val arg = buildJsonObject {
            put("text", text)
            put("apply", apply)
            put("tz", clock.utcOffsetSeconds())
            put("now", clock.nowSeconds())
        }
        val answer = engineJson.parseToJsonElement(port.call("import", arg.toString())).jsonObject
        answer["error"]?.let { throw EngineError(it.jsonPrimitive.content) }
        return answer
    }
}

/** For tests and previews: an engine that answers from a function. */
class FakePort(
    override val restored: Boolean = true,
    private val onSync: suspend () -> SyncResult = { SyncResult(false) },
    private val answer: (String, String) -> String = { _, _ -> "{}" },
) : EnginePort {
    var syncs = 0
        private set

    var forgotten = false
        private set

    /** The `switch` of the last sync. */
    var lastSwitch: String? = null
        private set

    override suspend fun call(op: String, arg: String) = answer(op, arg)

    override suspend fun sync(switch: String?): SyncResult {
        syncs++
        lastSwitch = switch
        return onSync()
    }

    override suspend fun forget() {
        forgotten = true
    }
}

internal fun JsonElement.string(): String = jsonPrimitive.content
