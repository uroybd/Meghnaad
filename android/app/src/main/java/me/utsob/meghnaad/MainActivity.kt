package me.utsob.meghnaad

import android.content.Intent
import android.os.Bundle
import com.getcapacitor.BridgeActivity

/** What a widget was tapped for, until the page takes it (see [TcEnginePlugin.getLaunch]): a task, or a report with a filter. */
object Launch {
    @Volatile
    var task: String? = null

    @Volatile
    var report: String? = null

    @Volatile
    var filter: String? = null

    /** Take what an intent asks for, if it asks for anything. */
    fun from(intent: Intent): Boolean {
        val task = intent.getStringExtra(MainActivity.EXTRA_TASK)
        val report = intent.getStringExtra(MainActivity.EXTRA_REPORT)
        if (task == null && report == null) return false
        this.task = task
        this.report = report
        this.filter = intent.getStringExtra(MainActivity.EXTRA_FILTER)
        return true
    }
}

class MainActivity : BridgeActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        registerPlugin(TcEnginePlugin::class.java)
        super.onCreate(savedInstanceState)
        Launch.from(intent)
    }

    /** The app is already open and a widget was tapped: tell the page, which asks what for. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (Launch.from(intent)) (bridge?.getPlugin("TcEngine")?.instance as? TcEnginePlugin)?.launchRequested()
    }

    companion object {
        const val EXTRA_TASK = "task"
        const val EXTRA_REPORT = "report"
        const val EXTRA_FILTER = "filter"
    }
}
