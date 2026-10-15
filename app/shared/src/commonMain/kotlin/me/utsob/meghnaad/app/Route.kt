package me.utsob.meghnaad.app

// A task's link, `meghnaad://task/<first 8 characters of its uuid>`: web/src/lib/route.ts, with the app's own scheme where the web has
// its address. (The numbers `task 3` uses differ from one replica to the next, so the uuid is how a task is named everywhere else.)

const val TASK_LINK_PREFIX = "meghnaad://task/"

private val TASK_LINK = Regex("^meghnaad://task/([0-9a-f][0-9a-f-]{7,35})/?$", RegexOption.IGNORE_CASE)

/** The link that opens this task. */
fun taskLink(uuid: String): String = TASK_LINK_PREFIX + uuid.take(8)

/** The uuid characters in a link like `meghnaad://task/1a2b3c4d`, or `null` when the text is not a task's link. */
fun prefixFromLink(link: String): String? = TASK_LINK.find(link)?.groupValues?.get(1)?.lowercase()

/**
 * The filter that finds a task by the start of its uuid. A bare `12345678` would be read as task numbers, so the attribute form
 * is used; it works for any prefix.
 */
fun prefixFilter(prefix: String): String = "uuid.startswith:$prefix"
