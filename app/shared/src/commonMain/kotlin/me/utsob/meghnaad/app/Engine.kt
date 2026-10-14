package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/** Where the engine is: native on every target (Rust, in the app's process). Every call answers in JSON text. */
interface EnginePort {
    /** `config`, `run`, `import`, `sync`, `changes` or `api`, with its argument as text. */
    suspend fun call(op: String, arg: String): String
}

/** What the page and the clock are, for a request: the offset east of UTC in seconds, and the time in seconds since 1970. */
interface Clock {
    fun utcOffsetSeconds(): Int

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
        return parseResponse(port.call("run", request.toString()))
    }
}

/** For tests and previews: an engine that answers from a function. */
class FakePort(private val answer: (String, String) -> String) : EnginePort {
    override suspend fun call(op: String, arg: String) = answer(op, arg)
}

internal fun JsonElement.string(): String = jsonPrimitive.content
