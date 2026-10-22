package me.utsob.meghnaad

import android.content.Context
import kotlinx.coroutines.runBlocking
import me.utsob.meghnaad.app.AppEngine
import me.utsob.meghnaad.app.Board
import me.utsob.meghnaad.app.R
import me.utsob.meghnaad.app.Section
import me.utsob.meghnaad.app.SectionData
import me.utsob.meghnaad.app.SystemClock
import me.utsob.meghnaad.app.Tw
import me.utsob.meghnaad.app.boardZone
import me.utsob.meghnaad.app.loadBoard
import me.utsob.meghnaad.app.orderChart
import me.utsob.meghnaad.app.parseSettings
import me.utsob.meghnaad.app.formatBoardValue
import me.utsob.meghnaad.app.placeLink
import me.utsob.meghnaad.app.AppPlace
import org.json.JSONObject

/** One card of a board on the home screen: a number or a chart, drawn from the engine's numbers, which opens the board when tapped. */
class BoardWidget : ImageWidget() {
    override fun render(context: Context, id: Int, width: Int, height: Int): Rendered {
        val config = WidgetPrefs.board(context, id)
        val none = context.getString(R.string.widget_board_choose)
        if (!config.chosen) return Rendered(context.getString(R.string.widget_board_label), message = none)
        val board = BoardsData.boards(context)?.firstOrNull { it.id == config.board }
            ?: return Rendered(context.getString(R.string.widget_board_label), message = context.getString(R.string.widget_board_gone))
        val section = board.sections.firstOrNull { it.id == config.section }
            ?: return Rendered(board.name, message = context.getString(R.string.widget_board_gone), link = placeLink(AppPlace.Boards(board.id)))
        val link = placeLink(AppPlace.Boards(board.id))
        if (section.type == "table") return Rendered(section.title, message = context.getString(R.string.widget_board_table), caption = board.name, link = link)

        val data = BoardsData.section(context, board, section)
            ?: return Rendered(section.title, message = context.getString(R.string.widget_not_ready), link = link)
        val look = WidgetPrefs.look(context, id)
        fun draw(palette: Palette) = when (data) {
            is SectionData.Number -> BoardRenderer.number(formatBoardValue(data.answer.total, data.answer.unit), data.answer.tasks, width, height, palette)
            is SectionData.Chart -> BoardRenderer.chart(orderChart(data.chart, section, data.limit), section, width, height, palette)
            else -> null
        }
        if (data is SectionData.Failed) return Rendered(section.title, message = data.message, caption = board.name, link = link)
        if (look.theme == "auto") {
            val light = draw(Palette(context, look.copy(theme = "light"))) ?: return Rendered(section.title, caption = board.name, link = link)
            return Rendered(section.title, light, draw(Palette(context, look.copy(theme = "dark"))), board.name, link = link)
        }
        val bitmap = draw(Palette(context, look)) ?: return Rendered(section.title, caption = board.name, link = link)
        return Rendered(section.title, bitmap, caption = board.name, link = link)
    }
}

/** The boards in `settings.json`, and a card's numbers, as the widgets ask the engine for them (they run off the main thread). */
object BoardsData {
    /** The boards the app has; null when the engine could not say. */
    fun boards(context: Context): List<Board>? = try {
        val answer = JSONObject(Engines.get(context).call("api", JSONObject().put("method", "GET").put("path", "/api/settings").toString()))
        val status = answer.optInt("status", 200)
        if (status in 200..299) parseSettings(answer.optString("body", "")).boards else emptyList()
    } catch (e: Exception) {
        android.util.Log.w("BoardWidget", "could not read the boards", e)
        null
    }

    /** The week starts on the day the taskrc says. */
    private fun weekstart(context: Context): String? = try {
        val answer = JSONObject(Engines.get(context).call("api", JSONObject().put("method", "GET").put("path", "/api/config").toString()))
        JSONObject(answer.getString("body")).getJSONObject("config").getJSONObject("settings").optString("weekstart").ifEmpty { null }
    } catch (e: Exception) {
        null
    }

    /** What one card shows now: only its own filters are run. Null when the engine had nothing to say. */
    fun section(context: Context, board: Board, section: Section): SectionData? = try {
        val settings = weekstart(context)?.let { mapOf("weekstart" to it) }
        val zone = boardZone(settings, java.util.TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 1000, System.currentTimeMillis() / 1000.0)
        runBlocking { loadBoard(Tw(AppEngine.get(context), SystemClock), board.copy(sections = listOf(section)), zone)[section.id] }
    } catch (e: Exception) {
        android.util.Log.w("BoardWidget", "could not work out the card", e)
        null
    }
}
