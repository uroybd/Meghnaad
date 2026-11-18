package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonPrimitive

/**
 * How a task's urgency adds up, as `task info` writes it (urgencyBreakdown.ts): the engine's terms, each a label, the
 * measure, the coefficient and what it adds, as text, then the total. The info screen draws them; nothing is worked out here.
 */
@Serializable
data class UrgencyBreakdown(val terms: List<List<String>>, val total: String)

fun urgencyBreakdown(task: Row, config: ConfigBody?, now: Long, tz: Int): UrgencyBreakdown =
    engineJson.decodeFromJsonElement(
        UrgencyBreakdown.serializer(),
        callHelper(
            "urgency_breakdown",
            engineJson.encodeToJsonElement(Row.serializer(), task),
            infoConfig(config),
            JsonPrimitive(now),
            JsonPrimitive(tz),
        ),
    )
