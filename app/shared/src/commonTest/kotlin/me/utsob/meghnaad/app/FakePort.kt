package me.utsob.meghnaad.app

/** For tests: an engine that answers from a function. */
class FakePort(
    override val restored: Boolean = true,
    private val onSync: suspend () -> SyncResult = { SyncResult(false) },
    private val answer: (String, String) -> String = { _, _ -> "{}" },
) : EnginePort {
    var syncs = 0
        private set

    var forgotten = false
        private set

    /** The `switch` of the last sync. */
    var lastSwitch: String? = null
        private set

    override suspend fun call(op: String, arg: String) = answer(op, arg)

    override suspend fun sync(switch: String?): SyncResult {
        syncs++
        lastSwitch = switch
        return onSync()
    }

    override suspend fun forget() {
        forgotten = true
    }
}
