package me.utsob.meghnaad

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.widget.Toast

/** What a tap in the widget's list lands on, invisibly: a tick marks the task done, a tap on the text opens that task in the app. */
class WidgetActionActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        when (intent.getStringExtra("kind")) {
            "done" -> {
                val uuid = intent.getStringExtra("uuid").orEmpty()
                val problem = try {
                    Tasks.done(this, uuid)
                } catch (e: Exception) {
                    e.message ?: "The task could not be finished."
                }
                if (problem != null) Toast.makeText(this, problem, Toast.LENGTH_LONG).show()
                Widgets.refreshAll(this)
                SyncWorker.runNow(this)
            }
            // The task that was tapped opens in the app, not just the app.
            else -> startActivity(
                Intent(this, MainActivity::class.java)
                    .putExtra(MainActivity.EXTRA_TASK, intent.getStringExtra("uuid"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        }
        finish()
    }
}
