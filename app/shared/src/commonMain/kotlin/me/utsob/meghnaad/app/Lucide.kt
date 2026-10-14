package me.utsob.meghnaad.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

// Lucide icons (https://lucide.dev), the same set as the web app: every icon travels with the app (resources/lucide.json,
// made by scripts/gen-lucide.mjs), because a task command can name any of them and the phone has no CDN to fetch from.
// An icon is a few shapes drawn with a 2-unit round stroke on a 24-unit square.

/** The bytes of a file packaged with the app (`/lucide.json`), or null. */
expect fun readResource(path: String): String?

/** One shape of an icon: its element name and attributes; paths and polygons are parsed once. */
class Shape(val tag: String, private val attrs: Map<String, String>) {
    private fun n(name: String) = attrs[name]?.toFloatOrNull() ?: 0f

    val path: Path? by lazy {
        when (tag) {
            "path" -> attrs["d"]?.let { runCatching { PathParser().parsePathString(it).toPath() }.getOrNull() }
            "polyline", "polygon" -> attrs["points"]?.let { points ->
                val xs = points.trim().split(Regex("[\\s,]+")).mapNotNull { it.toFloatOrNull() }
                Path().apply {
                    xs.chunked(2).filter { it.size == 2 }.forEachIndexed { i, p -> if (i == 0) moveTo(p[0], p[1]) else lineTo(p[0], p[1]) }
                    if (tag == "polygon") close()
                }
            }
            else -> null
        }
    }

    fun draw(scope: androidx.compose.ui.graphics.drawscope.DrawScope, color: Color, stroke: Stroke) = with(scope) {
        when (tag) {
            "circle" -> drawCircle(color, n("r"), Offset(n("cx"), n("cy")), style = stroke)
            "ellipse" -> drawOval(color, Offset(n("cx") - n("rx"), n("cy") - n("ry")), Size(2 * n("rx"), 2 * n("ry")), style = stroke)
            "line" -> drawLine(color, Offset(n("x1"), n("y1")), Offset(n("x2"), n("y2")), stroke.width, stroke.cap)
            "rect" -> {
                val rx = n("rx").let { if (it == 0f) n("ry") else it }
                if (rx > 0f) drawRoundRect(color, Offset(n("x"), n("y")), Size(n("width"), n("height")), CornerRadius(rx), stroke)
                else drawRect(color, Offset(n("x"), n("y")), Size(n("width"), n("height")), style = stroke)
            }
            else -> path?.let { drawPath(it, color, style = stroke) }
        }
    }
}

/** Every icon the app carries, and the words each is found by (loaded once, off the main thread). */
object LucideLib {
    private var icons by mutableStateOf<Map<String, List<Shape>>>(emptyMap())

    /** The search words of each icon, for the picker; empty until loaded. */
    var tags: Map<String, List<String>> = emptyMap()
        private set

    var loaded by mutableStateOf(false)
        private set

    suspend fun load() {
        if (loaded) return
        val parsed = withContext(Dispatchers.Default) {
            val text = readResource("/lucide.json") ?: return@withContext null
            val root = engineJson.parseToJsonElement(text).jsonObject
            val shapes = root.getValue("icons").jsonObject.mapValues { (_, v) ->
                v.jsonArray.map { s ->
                    val tag = s.jsonArray[0].jsonPrimitive.content
                    Shape(tag, s.jsonArray[1].jsonObject.mapValues { (_, a) -> a.jsonPrimitive.content })
                }
            }
            val words = (root["tags"] as? JsonObject)?.mapValues { (_, v) -> v.jsonArray.map { it.jsonPrimitive.content } } ?: emptyMap()
            shapes to words
        } ?: return
        tags = parsed.second
        icons = parsed.first
        loaded = true
    }

    /** The shapes of an icon by its name (`circle-x`), or null while loading or when there is no such icon. */
    fun shapes(name: String): List<Shape>? = icons[name]

    fun has(name: String): Boolean = name in icons

    /** All the names, sorted. */
    fun names(): List<String> = icons.keys.sorted()
}

/** An icon from the set, in the colour of the text around it unless told otherwise. */
@Composable
fun LucideIcon(
    name: String,
    modifier: Modifier = Modifier,
    size: Dp = 18.dp,
    tint: Color = LocalContentColor.current,
    strokeWidth: Float = 2f,
) {
    val shapes = LucideLib.shapes(name)
    val stroke = remember(strokeWidth) { Stroke(width = strokeWidth, cap = StrokeCap.Round, join = StrokeJoin.Round) }
    Canvas(modifier.size(size)) {
        if (shapes == null) return@Canvas
        scale(this.size.width / 24f, pivot = Offset.Zero) { shapes.forEach { it.draw(this, tint, stroke) } }
    }
}
