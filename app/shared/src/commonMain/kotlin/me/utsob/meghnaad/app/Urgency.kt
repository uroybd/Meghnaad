package me.utsob.meghnaad.app

// Urgency settings as the Urgency tab shows them (web/src/lib/urgency.ts). The key spelling and what each key applies to
// follow Taskwarrior's `urgency.*` settings; the engine re-checks every key before saving.

enum class UrgencyKind { Term, Project, Tag, Keyword, Uda }

data class Described(val key: String, val kind: UrgencyKind, val name: String, val label: String, val hint: String)

class UrgencyTerm(val term: String, val label: String, val hint: String)

/** The built-in terms, in the order Taskwarrior lists them. */
val URGENCY_TERMS = listOf(
    UrgencyTerm("project", "Has a project", "The task belongs to a project."),
    UrgencyTerm("active", "Is started", "The task has been started and is active."),
    UrgencyTerm("scheduled", "Scheduled date has passed", "The scheduled date is in the past."),
    UrgencyTerm("waiting", "Is waiting", "The wait date is still in the future."),
    UrgencyTerm("blocked", "Is blocked", "Depends on a task that is not done yet."),
    UrgencyTerm("annotations", "Has annotations", "Scales with the count: 1 → 0.8, 2 → 0.9, 3 or more → 1.0."),
    UrgencyTerm("tags", "Has tags", "Scales with the count: 1 → 0.8, 2 → 0.9, 3 or more → 1.0."),
    UrgencyTerm("due", "Due date", "Rises as the date nears: 0.2 two weeks out, 1.0 a week overdue."),
    UrgencyTerm("blocking", "Blocks another task", "Another pending task depends on this one."),
    UrgencyTerm("age", "Age", "Grows from 0 to 1 over the days set by the age cap below."),
)

const val AGE_MAX = "urgency.age.max"
const val INHERIT_KEY = "urgency.inherit"
private const val SUFFIX = ".coefficient"

fun describeUrgency(key: String): Described {
    val rest = key.removePrefix("urgency.")
    if (key == AGE_MAX) {
        return Described(key, UrgencyKind.Term, "age.max", "Age cap (days)", "A task this old or older gets the full age value.")
    }
    val body = if (rest.endsWith(SUFFIX)) rest.dropLast(SUFFIX.length) else rest
    URGENCY_TERMS.firstOrNull { it.term == body }?.let { return Described(key, UrgencyKind.Term, it.term, it.label, it.hint) }

    fun user(prefix: String) = if (body.startsWith(prefix)) body.substring(prefix.length) else null
    user("user.project.")?.let {
        return Described(key, UrgencyKind.Project, it, "Project $it", "Applies to $it and every sub-project ($it.*).")
    }
    user("user.tag.")?.let {
        return Described(key, UrgencyKind.Tag, it, "Tag +$it", "Tasks tagged $it. Virtual tags such as OVERDUE work too.")
    }
    user("user.keyword.")?.let {
        return Described(key, UrgencyKind.Keyword, it, "Keyword “$it”", "The description contains this text (case-sensitive).")
    }
    val uda = user("uda.") ?: if (body.startsWith("uda.")) body.substring(4) else null
    if (uda != null) {
        val parts = uda.split('.')
        val name = parts[0]
        val value = parts.drop(1).joinToString(".")
        return if (parts.size > 1) {
            Described(key, UrgencyKind.Uda, uda, "$name = $value", "The $name attribute is exactly $value.")
        } else {
            Described(key, UrgencyKind.Uda, uda, "$name has a value", "The $name attribute is set, to anything.")
        }
    }
    return Described(key, UrgencyKind.Term, rest, key, "")
}

class Addable(val kind: UrgencyKind, val label: String, val placeholder: String)

/** What can be added beyond the built-in terms. */
val ADDABLE = listOf(
    Addable(UrgencyKind.Project, "Project", "Home"),
    Addable(UrgencyKind.Tag, "Tag", "next"),
    Addable(UrgencyKind.Keyword, "Keyword", "urgent"),
    Addable(UrgencyKind.Uda, "UDA (any value)", "estimate"),
    Addable(UrgencyKind.Uda, "UDA = value", "estimate.huge"),
)

/** The same rule the engine applies to names: no spaces, `=`, `#`, and not containing ".coefficient". */
fun validUrgencyName(name: String) =
    name.isNotEmpty() && !name.contains(SUFFIX) && name.none { it.isWhitespace() || it == '=' || it == '#' || it.code < 0x20 }

fun buildUrgencyKey(kind: UrgencyKind, name: String): String {
    val prefix = when (kind) {
        UrgencyKind.Project -> "urgency.user.project."
        UrgencyKind.Tag -> "urgency.user.tag."
        UrgencyKind.Keyword -> "urgency.user.keyword."
        else -> "urgency.uda."
    }
    return "$prefix$name$SUFFIX"
}

/** UDA names are plain identifiers; the value after the first dot is free text. */
fun validUda(name: String): Boolean {
    val parts = name.split('.')
    return parts[0].isNotEmpty() && parts[0].all { it.isLetterOrDigit() && it.code < 128 || it == '_' } &&
        (parts.size == 1 || validUrgencyName(parts.drop(1).joinToString(".")))
}

/** The order rows appear in: built-in terms first, then custom ones grouped by kind. */
fun sortUrgencyKeys(keys: Collection<String>): List<String> {
    fun rank(k: String): Int {
        val d = describeUrgency(k)
        return when {
            k == AGE_MAX -> URGENCY_TERMS.size
            d.kind == UrgencyKind.Term -> URGENCY_TERMS.indexOfFirst { it.term == d.name }
            else -> 100 + listOf(UrgencyKind.Project, UrgencyKind.Tag, UrgencyKind.Keyword, UrgencyKind.Uda).indexOf(d.kind)
        }
    }
    return keys.sortedWith(compareBy<String> { rank(it) }.thenBy { it })
}

/** The value in force: the saved override, else Taskwarrior's built-in one. */
fun effectiveUrgency(key: String, overrides: Map<String, Double>, defaults: Map<String, Double>): Double? = overrides[key] ?: defaults[key]

/** Record `value` for `key`. A value equal to the built-in one is not an override, so it is dropped and the setting follows the default again. */
fun withUrgencyValue(overrides: Map<String, Double>, defaults: Map<String, Double>, key: String, value: Double): Map<String, Double> {
    val next = overrides.toMutableMap()
    if (defaults[key] == value) next.remove(key) else next[key] = value
    return next
}
