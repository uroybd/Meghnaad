package me.utsob.meghnaad.app

// What `priority` can be (priority.ts): Taskwarrior defines it as a UDA (`uda.priority.values`, `H,M,L,` unless the taskrc says otherwise), so
// a taskrc may rename the levels (`Critical,Important,`) or put the blank between two of them.

/** The values `priority` takes, highest first, the blank one left out. */
fun priorityValues(udas: Map<String, UdaDef>): List<String> =
    udas["priority"]?.values?.filter { it.isNotEmpty() }?.takeIf { it.isNotEmpty() } ?: listOf("H", "M", "L")

/** The values as a sentence: `H, M or L`. */
fun orList(values: List<String>): String =
    if (values.size > 1) "${values.dropLast(1).joinToString(", ")} or ${values.last()}" else values.firstOrNull().orEmpty()
