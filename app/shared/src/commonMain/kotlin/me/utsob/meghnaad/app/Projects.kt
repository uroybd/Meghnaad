package me.utsob.meghnaad.app

// Grouping tasks into the project tree. `Home.Kitchen` is a child of `Home`, as in Taskwarrior (web/src/lib/projects.ts).

class ProjectNode(
    /** Full dotted name; empty is the group of tasks that have no project. */
    val name: String,
    /** Last segment, for display. */
    var label: String,
    /** Tasks filed exactly here, most urgent first. */
    val own: MutableList<Row> = mutableListOf(),
    val children: MutableList<ProjectNode> = mutableListOf(),
    /** Tasks here and in every sub-project. */
    var total: Int = 0,
    /** Of those, how many are past due. */
    var overdue: Int = 0,
)

const val NO_PROJECT = ""

private val byName = compareBy<ProjectNode, String>(String.CASE_INSENSITIVE_ORDER) { it.name }

/**
 * Build the tree from the tasks given (the page asks for those its filter matches, finished ones included when the filter
 * allows them, so a project stays listed after its last task is done). Sub-projects with no tasks of their own still get a node,
 * and so does [ensure]: a project asked for from a chip that no task in [rows] has. Only pending tasks count as overdue.
 */
fun buildTree(rows: List<Row>, now: Long, ensure: String? = null): List<ProjectNode> {
    val nodes = linkedMapOf<String, ProjectNode>()
    fun node(name: String): ProjectNode = nodes.getOrPut(name) {
        ProjectNode(name, name.substring(name.lastIndexOf('.') + 1))
    }.also { n ->
        val cut = name.lastIndexOf('.')
        if (cut > 0) node(name.substring(0, cut)).let { parent -> if (n !in parent.children) parent.children += n }
    }

    for (r in rows) {
        val n = node(r.project ?: NO_PROJECT)
        n.own += r
        val late = r.status == "pending" && r.due != null && r.due < now
        // Count on the node and on each ancestor.
        var name = n.name
        while (true) {
            val m = nodes.getValue(name)
            m.total++
            if (late) m.overdue++
            if ('.' !in name) break
            name = name.substring(0, name.lastIndexOf('.'))
        }
    }

    if (ensure != null) node(ensure)

    val byUrgency = compareByDescending<Row> { it.urgency }
    fun sort(n: ProjectNode) {
        n.own.sortWith(byUrgency)
        n.children.sortWith(byName)
        n.children.forEach(::sort)
    }
    val roots = nodes.values.filter { '.' !in it.name && it.name != NO_PROJECT }.sortedWith(byName).toMutableList()
    roots.forEach(::sort)
    nodes[NO_PROJECT]?.let { none ->
        none.own.sortWith(byUrgency)
        none.label = "(no project)"
        roots += none
    }
    return roots
}

/** Every node that has something to expand, for "expand all". */
fun expandable(tree: List<ProjectNode>): List<String> =
    tree.flatMap { n -> (if (n.children.isNotEmpty() || n.own.isNotEmpty()) listOf(n.name) else emptyList()) + expandable(n.children) }
