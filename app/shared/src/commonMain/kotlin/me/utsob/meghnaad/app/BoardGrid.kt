package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt

// A board on the phone (BoardView.svelte on a narrow screen): the sections on 4 columns (6 on a wide phone or a tablet), each as wide
// as its share of the row, so several numbers sit side by side. Where each one goes is the engine's (`flowed`). To edit the layout the
// board is drawn the same way with a card to tap, and a corner to drag to resize it (`resizeFlow`, which keeps the width in the board's twelve).

private val ROW = 56.dp
private val GAP = 8.dp

/** What editing a board's layout needs from its grid: which card is picked, and what a tap and a drag of a corner do. */
class GridEdit(
    val selected: String?,
    val onSelect: (String) -> Unit,
    val onLayout: (List<Section>) -> Unit,
)

@Composable
fun BoardGrid(store: Store, sections: List<Section>, data: Map<String, SectionData>, onInfo: (String) -> Unit, edit: GridEdit? = null) {
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val cols = phoneColumns(maxWidth.value)
        val cell = (maxWidth - GAP * (cols - 1)) / cols
        val placed = remember(sections, cols) { flowed(sections, cols) }
        val rows = placed.maxOfOrNull { it.y + it.h } ?: 0
        Box(Modifier.fillMaxWidth().height(if (rows == 0) 0.dp else ROW * rows + GAP * (rows - 1))) {
            placed.forEach { s ->
                key(s.id) {
                    val width = cell * s.w + GAP * (s.w - 1)
                    val height = ROW * s.h + GAP * (s.h - 1)
                    Box(Modifier.offset(x = (cell + GAP) * s.x, y = (ROW + GAP) * s.y).size(width, height)) {
                        BoardCard(store, s, data[s.id], width, height, showInfo = edit == null) { onInfo(s.id) }
                        if (edit != null) EditOverlay(s, sections, cols, cell, edit)
                    }
                }
            }
        }
    }
}

/** Over a card being edited: a tap picks it, and a picked one has a corner to drag. */
@Composable
private fun EditOverlay(s: Section, sections: List<Section>, cols: Int, cell: Dp, edit: GridEdit) {
    val colors = MaterialTheme.colorScheme
    val picked = edit.selected == s.id
    val shape = RoundedCornerShape(12.dp)
    val density = LocalDensity.current
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier.fillMaxSize()
                .then(if (picked) Modifier.border(2.dp, colors.primary, shape) else Modifier.border(1.dp, colors.outline.copy(alpha = 0.6f), shape))
                .clip(shape)
                .clickable { edit.onSelect(s.id) }
                .semantics { contentDescription = "${s.title}. ${if (picked) "Picked." else "Tap to pick."}" },
        )
        if (picked) {
            // The layout as it was when the drag began, so dragging back puts what was pushed aside back where it was.
            val drag = remember { CornerDrag() }
            val now by rememberUpdatedState(sections)
            Box(
                Modifier.align(Alignment.BottomEnd).size(36.dp)
                    .semantics { contentDescription = "Resize ${s.title}. Drag." }
                    .pointerInput(s.id, cols, cell) {
                        detectDragGestures(
                            onDragStart = { drag.begin(now) },
                            onDrag = { change, amount ->
                                change.consume()
                                val step = drag.move(amount.x, amount.y, with(density) { (cell + GAP).toPx() }, with(density) { (ROW + GAP).toPx() })
                                if (step != null) edit.onLayout(resizeFlow(drag.base, s.id, cols, step.first, step.second))
                            },
                        )
                    },
                contentAlignment = Alignment.BottomEnd,
            ) {
                Box(Modifier.padding(6.dp).size(18.dp).clip(RoundedCornerShape(topStart = 18.dp, topEnd = 2.dp, bottomEnd = 8.dp, bottomStart = 2.dp)).background(colors.primary))
            }
        }
    }
}

/** A drag of a card's corner: the layout it began with, how far it has gone, and the step that makes (columns, rows). */
private class CornerDrag {
    var base: List<Section> = emptyList()
    private var dx = 0f
    private var dy = 0f
    private var last = 0 to 0

    fun begin(sections: List<Section>) {
        base = sections
        dx = 0f
        dy = 0f
        last = 0 to 0
    }

    /** The step after moving by ([x], [y]) pixels, or null if it is still the one before. */
    fun move(x: Float, y: Float, stepX: Float, stepY: Float): Pair<Int, Int>? {
        dx += x
        dy += y
        val step = (dx / stepX).roundToInt() to (dy / stepY).roundToInt()
        if (step == last) return null
        last = step
        return step
    }
}

/** One section: a number, a table or a chart, filling the place it has. What does not fit is scrolled. */
@Composable
private fun BoardCard(store: Store, section: Section, data: SectionData?, width: Dp, height: Dp, showInfo: Boolean, onInfo: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Card(Modifier.fillMaxSize()) {
        Column(Modifier.padding(start = 10.dp, top = 2.dp, end = 2.dp, bottom = 6.dp)) {
            Row(Modifier.height(34.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(section.title, Modifier.weight(1f), fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = colors.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                if (showInfo) IconAction("info", "Information about ${section.title}", onInfo, enabled = data != null, tint = colors.onSurfaceVariant, size = 15.dp, box = 34.dp)
            }
            Column(Modifier.weight(1f).fillMaxWidth().padding(end = 8.dp).verticalScroll(rememberScrollState())) {
                when (data) {
                    null -> Dim("Loading…")
                    is SectionData.Failed -> Text(data.message, color = colors.error, fontSize = 13.sp)
                    is SectionData.Number -> {
                        // A quarter of a row is narrow: its figure is smaller.
                        val big = when {
                            width < 100.dp -> 22.sp
                            width < 160.dp -> 28.sp
                            else -> 34.sp
                        }
                        Text(formatBoardValue(data.answer.total, data.answer.unit), fontSize = big, fontWeight = FontWeight.Bold, maxLines = 1)
                        Dim("${data.answer.tasks} ${if (data.answer.tasks == 1) "task" else "tasks"}", size = if (width < 100.dp) 11 else 13)
                    }
                    is SectionData.Chart -> BoardChartView(orderChart(data.chart, section), section, extras.dark, height = height - 34.dp - 8.dp)
                    is SectionData.Table -> {
                        val e = remember(data) { Entry(0, CliInput(args = emptyList()), section.report ?: "next").also { it.result = data.result } }
                        ResultBlock(store, e)
                    }
                }
            }
        }
    }
}
