package me.utsob.meghnaad

import me.utsob.meghnaad.app.R
import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.Intent
import android.os.Bundle
import android.text.InputType
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.SeekBar
import android.widget.Spinner
import android.widget.TextView

/**
 * Asked for by the system when a widget is added to the home screen (and again from the widget's own settings): which
 * report a list shows and how, which period a burndown covers, and a filter to narrow what any of them look at.
 * Backing out leaves the widget unplaced.
 */
class WidgetConfigActivity : Activity() {
    private var id = AppWidgetManager.INVALID_APPWIDGET_ID
    private lateinit var form: LinearLayout

    private fun dp(n: Int) = (n * resources.displayMetrics.density).toInt()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setResult(RESULT_CANCELED)
        id = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
        val kind = Widgets.kindOf(this, id)
        if (id == AppWidgetManager.INVALID_APPWIDGET_ID || kind == null) return finish()

        form = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(20), dp(16), dp(20), dp(8))
        }
        setContentView(ScrollView(this).apply { addView(form) })
        when (kind) {
            "list" -> listForm()
            "burndown" -> burndownForm()
            "board" -> boardForm()
            else -> calendarForm()
        }
    }

    private fun heading(text: String) = form.addView(TextView(this).apply {
        this.text = text
        textSize = 20f
        setPadding(0, 0, 0, dp(8))
    })

    private fun label(text: String) = form.addView(TextView(this).apply {
        this.text = text
        textSize = 13f
        alpha = 0.7f
        setPadding(0, dp(14), 0, dp(2))
    })

    private fun field(hint: String, value: String, number: Boolean = false): EditText {
        val edit = EditText(this).apply {
            this.hint = hint
            setText(value)
            setSingleLine()
            inputType = if (number) InputType.TYPE_CLASS_NUMBER else InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            minHeight = dp(48)
        }
        form.addView(edit, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        return edit
    }

    private fun choice(items: List<String>, selected: String): Spinner {
        val spinner = Spinner(this)
        spinner.adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, items.toMutableList())
        spinner.setSelection(items.indexOf(selected).coerceAtLeast(0))
        form.addView(spinner, ViewGroup.LayoutParams.MATCH_PARENT, dp(48))
        return spinner
    }

    /** Which theme, and how see-through the background is: for every kind of widget. Returns what the form now says. */
    private fun lookForm(): () -> WidgetLook {
        val current = WidgetPrefs.look(this, id)
        label("Theme")
        val names = listOf("Follow the phone", "Light", "Dark")
        val theme = Spinner(this)
        theme.adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, names)
        theme.setSelection(WidgetLook.THEMES.indexOf(current.theme).coerceAtLeast(0))
        form.addView(theme, ViewGroup.LayoutParams.MATCH_PARENT, dp(48))
        val opacityLabel = TextView(this).apply {
            textSize = 13f
            alpha = 0.7f
            setPadding(0, dp(14), 0, dp(2))
        }
        form.addView(opacityLabel)
        val opacity = SeekBar(this).apply {
            max = 100
            progress = current.opacity
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(bar: SeekBar, value: Int, fromUser: Boolean) {
                    opacityLabel.text = if (value == 0) "Background: see-through" else "Background opacity: $value%"
                }

                override fun onStartTrackingTouch(bar: SeekBar) {}

                override fun onStopTrackingTouch(bar: SeekBar) {}
            })
        }
        opacityLabel.text = if (current.opacity == 0) "Background: see-through" else "Background opacity: ${current.opacity}%"
        form.addView(opacity, ViewGroup.LayoutParams.MATCH_PARENT, dp(40))
        return { WidgetLook(WidgetLook.THEMES[theme.selectedItemPosition], opacity.progress) }
    }

    private fun buttons(onSave: () -> Unit) {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = android.view.Gravity.END
            setPadding(0, dp(20), 0, dp(8))
        }
        row.addView(Button(this, null, android.R.attr.buttonBarButtonStyle).apply {
            text = getString(android.R.string.cancel)
            setOnClickListener { finish() }
        })
        row.addView(Button(this, null, android.R.attr.buttonBarButtonStyle).apply {
            text = getString(R.string.widget_save)
            setOnClickListener {
                onSave()
                Widgets.update(this@WidgetConfigActivity, id)
                setResult(RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id))
                finish()
            }
        })
        form.addView(row)
    }

    private fun listForm() {
        val current = WidgetPrefs.list(this, id)
        heading(getString(R.string.widget_tasks_label))
        label("Report")
        val report = choice(listOf(current.report), current.report)
        // The report names come from the engine, which may take a moment the first time.
        Thread {
            val names = Tasks.reportNames(this).let { if (current.report in it) it else listOf(current.report) + it }
            runOnUiThread {
                report.adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, names)
                report.setSelection(names.indexOf(current.report).coerceAtLeast(0))
            }
        }.start()
        label("Filter (optional)")
        val filter = field("project:Home +work due.before:eow", current.filter)
        label("Title (optional)")
        // Left empty, the widget is called what its report is described as.
        val title = field(current.description.ifBlank { current.report }, current.title)
        label("Most tasks to show")
        val limit = field("12", current.limit.toString(), number = true)
        label("Show under each task")
        val boxes = ListFields.ALL.map { (key, name) ->
            CheckBox(this).apply {
                text = name
                isChecked = key in current.fields
                minHeight = dp(44)
            }.also { form.addView(it) }
        }
        val look = lookForm()
        buttons {
            WidgetPrefs.saveLook(this, id, look())
            WidgetPrefs.saveList(
                this,
                id,
                ListConfig(
                    report.selectedItem?.toString() ?: current.report,
                    filter.text.toString().trim(),
                    limit.text.toString().toIntOrNull()?.coerceIn(1, 50) ?: 12,
                    ListFields.ALL.filterIndexed { i, _ -> boxes[i].isChecked }.map { it.first },
                    title.text.toString().trim(),
                    // The description is of the report: it is looked up again once the widget is drawn.
                    if ((report.selectedItem?.toString() ?: current.report) == current.report) current.description else "",
                ),
            )
        }
    }

    private fun burndownForm() {
        val current = WidgetPrefs.burndown(this, id)
        heading(getString(R.string.widget_burndown_label))
        label("Bars are")
        val period = choice(BurndownConfig.PERIODS, current.period)
        label("Only the tasks that match (optional)")
        val filter = field("project:Home", current.filter)
        val look = lookForm()
        buttons {
            WidgetPrefs.saveLook(this, id, look())
            WidgetPrefs.saveBurndown(this, id, BurndownConfig(period.selectedItem.toString(), filter.text.toString().trim()))
        }
    }

    private fun boardForm() {
        val current = WidgetPrefs.board(this, id)
        heading(getString(R.string.widget_board_label))
        label("Board")
        val board = choice(listOf(getString(R.string.widget_nothing_to_show)), "")
        label("Card")
        val card = choice(listOf(""), "")
        var boards: List<me.utsob.meghnaad.app.Board> = emptyList()
        fun showCards(selected: String?) {
            val sections = boards.getOrNull(board.selectedItemPosition)?.sections.orEmpty()
            val names = sections.map { it.title.ifBlank { it.id } + if (it.type == "table") " (a table is not drawn)" else "" }
            card.adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, names.ifEmpty { listOf("") })
            card.setSelection(sections.indexOfFirst { it.id == selected }.coerceAtLeast(0))
        }
        board.onItemSelectedListener = object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: View?, position: Int, itemId: Long) = showCards(current.section.takeIf { boards.getOrNull(position)?.id == current.board })

            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        }
        // The boards are read from the engine, which may take a moment the first time.
        Thread {
            val found = BoardsData.boards(this).orEmpty()
            runOnUiThread {
                boards = found
                val names = found.map { it.name }.ifEmpty { listOf(getString(R.string.widget_nothing_to_show)) }
                board.adapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, names)
                board.setSelection(found.indexOfFirst { it.id == current.board }.coerceAtLeast(0))
                showCards(current.section)
            }
        }.start()
        form.addView(TextView(this).apply {
            text = "A number is drawn large; a chart is drawn as in the app. Make the widget bigger for more detail. Tap it to open the board."
            textSize = 13f
            alpha = 0.7f
            setPadding(0, dp(12), 0, 0)
        })
        val look = lookForm()
        buttons {
            WidgetPrefs.saveLook(this, id, look())
            val chosen = boards.getOrNull(board.selectedItemPosition)
            val section = chosen?.sections?.getOrNull(card.selectedItemPosition)
            WidgetPrefs.saveBoard(this, id, BoardWidgetConfig(chosen?.id.orEmpty(), section?.id.orEmpty()))
        }
    }

    private fun calendarForm() {
        val current = WidgetPrefs.calendar(this, id)
        heading(getString(R.string.widget_calendar_label))
        label("Only the tasks that match (optional)")
        val filter = field("project:Work", current.filter)
        form.addView(TextView(this).apply {
            text = "Days with something due get a dot: red when overdue, amber today, blue otherwise. Tap a day to select it; make the widget bigger to see that day's tasks beside the month or under it."
            textSize = 13f
            alpha = 0.7f
            setPadding(0, dp(12), 0, 0)
        })
        val look = lookForm()
        buttons {
            WidgetPrefs.saveLook(this, id, look())
            WidgetPrefs.saveCalendar(this, id, CalendarConfig(filter.text.toString().trim()))
        }
    }
}
