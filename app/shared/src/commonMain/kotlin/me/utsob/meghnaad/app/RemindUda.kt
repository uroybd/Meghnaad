package me.utsob.meghnaad.app

// The `remind_in` UDA (web/src/lib/remindUda.ts): a task's own times before its due and scheduled dates (`10,1h`). Reminders
// work without it, from the setting; defining it lets `add … remind_in:10,1h` keep the value and shows it on the task.

const val REMIND_UDA = "remind_in"

/** [Part.Ok]: a string UDA. [Part.Different]: defined with another type. */
fun remindUdaStatus(config: ConfigResponse?): Part {
    val uda = config?.config?.udas?.get(REMIND_UDA)
    return when {
        uda == null -> Part.Missing
        uda.type == "string" -> Part.Ok
        else -> Part.Different
    }
}

/** The `config` lines (name, value) that define it; none when it is already so. The label stays when there is one. */
fun remindUdaLines(config: ConfigResponse?): List<Pair<String, String>> {
    if (remindUdaStatus(config) == Part.Ok) return emptyList()
    val lines = mutableListOf("uda.$REMIND_UDA.type" to "string")
    if (config?.config?.udas?.get(REMIND_UDA)?.label.isNullOrEmpty()) lines.add("uda.$REMIND_UDA.label" to "Remind in")
    return lines
}
