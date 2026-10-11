package me.utsob.meghnaad

import android.app.Activity
import android.os.Bundle
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.TextView

/** A line to type a task into, from the widget's + button: `Buy milk due:tomorrow +home`, as in the console. */
class QuickAddActivity : Activity() {
    private lateinit var line: EditText
    private lateinit var problem: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_quick_add)
        line = findViewById(R.id.line)
        problem = findViewById(R.id.problem)
        findViewById<android.view.View>(R.id.cancel).setOnClickListener { finish() }
        findViewById<android.view.View>(R.id.save).setOnClickListener { save() }
        line.setOnEditorActionListener { _, action, _ ->
            if (action == EditorInfo.IME_ACTION_DONE) save()
            action == EditorInfo.IME_ACTION_DONE
        }
    }

    private fun save() {
        val text = line.text.toString().trim()
        if (text.isEmpty()) return finish()
        Thread {
            val failed = try {
                Tasks.add(this, text)
            } catch (e: Exception) {
                e.message ?: "The task could not be added."
            }
            runOnUiThread {
                if (failed != null) {
                    problem.text = failed
                    problem.visibility = android.view.View.VISIBLE
                } else {
                    Widgets.refreshAll(this)
                    SyncWorker.runNow(this)
                    finish()
                }
            }
        }.start()
    }
}
