package me.utsob.meghnaad

import android.os.Bundle
import com.getcapacitor.BridgeActivity

class MainActivity : BridgeActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        registerPlugin(TcEnginePlugin::class.java)
        super.onCreate(savedInstanceState)
    }
}
