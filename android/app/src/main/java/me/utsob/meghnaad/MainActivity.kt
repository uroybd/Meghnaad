package me.utsob.meghnaad

import android.content.Intent
import android.os.Bundle
import com.getcapacitor.BridgeActivity

/** The task a widget was tapped on, until the page takes it (see [TcEnginePlugin.getLaunchTask]). */
object Launch {
    @Volatile
    var task: String? = null
}

class MainActivity : BridgeActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        registerPlugin(TcEnginePlugin::class.java)
        super.onCreate(savedInstanceState)
        Launch.task = intent.getStringExtra(EXTRA_TASK) ?: Launch.task
    }

    /** The app is already open and a widget was tapped: tell the page, which asks for the task. */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val task = intent.getStringExtra(EXTRA_TASK) ?: return
        Launch.task = task
        (bridge?.getPlugin("TcEngine")?.instance as? TcEnginePlugin)?.taskRequested()
    }

    companion object {
        const val EXTRA_TASK = "task"
    }
}
