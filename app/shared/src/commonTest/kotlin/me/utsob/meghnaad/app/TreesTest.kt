package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class TreesTest {
    private fun row(description: String, project: String?, status: String = "pending", urgency: Double = 1.0, due: Long? = null, tags: List<String> = emptyList(), uuid: String = description) =
        Row(uuid = uuid, status = status, description = description, project = project, urgency = urgency, due = due, tags = tags)

    private val rows = listOf(
        row("a", "Home"),
        row("b", "Home.Kitchen", due = 50),
        row("c", "Home.Kitchen", urgency = 9.0),
        row("d", "Work.Deep.Nest"),
        row("e", null),
        row("done", "Home", status = "completed"),
    )
    private val tree = buildTree(rows, 100)

    @Test
    fun nestsDottedNamesCreatingParentsThatHoldNoTasks() {
        assertEquals(listOf("Home", "Work", ""), tree.map { it.name })
        val work = tree[1]
        assertEquals(emptyList(), work.own)
        assertEquals("Work.Deep", work.children[0].name)
        assertEquals("Work.Deep.Nest", work.children[0].children[0].name)
    }

    @Test
    fun countsTheTasksGivenSubProjectsIncludedFinishedOnesToo() {
        assertEquals(4, tree[0].total)
        assertEquals(listOf("a", "done"), tree[0].own.map { it.description })
        assertEquals(1, tree[1].total)
    }

    @Test
    fun keepsAProjectListedWhenOnlyFinishedTasksAreLeft() {
        val t = buildTree(listOf(row("x", "Old", status = "completed", due = 1)), 100)
        assertEquals(listOf(Triple("Old", 1, 0)), t.map { Triple(it.name, it.total, it.overdue) })
    }

    @Test
    fun countsOverdueTasksUpTheTree() {
        assertEquals(1, tree[0].overdue)
        assertEquals(1, tree[0].children[0].overdue)
        assertEquals(0, tree[1].overdue)
    }

    @Test
    fun putsTheMostUrgentTaskFirstAndTheNoProjectGroupLast() {
        assertEquals(listOf("c", "b"), tree[0].children[0].own.map { it.description })
        assertEquals("(no project)", tree.last().label)
    }

    @Test
    fun listsWhatCanBeExpanded() {
        assertTrue("Work.Deep" in expandable(tree))
        assertEquals(emptyList(), expandable(emptyList()))
    }

    @Test
    fun addsTheProjectAskedForAndItsParentsEvenWhenNoTaskIsThere() {
        val t = buildTree(listOf(row("a", "Work")), 100, "Home.Kitchen")
        assertEquals(listOf("Home", "Work"), t.map { it.name })
        assertEquals(listOf("Home.Kitchen" to 0), t[0].children.map { it.name to it.total })
    }

    @Test
    fun leavesAProjectThatIsAlreadyThereAsItWas() {
        val t = buildTree(listOf(row("a", "Home.Kitchen")), 100, "Home.Kitchen")
        assertEquals(1, t[0].children[0].total)
    }

    @Test
    fun groupsTasksUnderEachOfTheirTagsMostUrgentFirstFinishedOnesIncluded() {
        val rs = listOf(
            row("1", null, tags = listOf("b", "a"), urgency = 1.0),
            row("2", null, tags = listOf("a"), urgency = 5.0, due = 10),
            row("3", null, tags = listOf("a"), status = "completed", urgency = 0.0),
        )
        val t = buildTags(rs, 100)
        assertEquals(listOf("a", "b"), t.map { it.name })
        assertEquals(listOf("2", "1", "3"), t[0].tasks.map { it.uuid })
        assertEquals(1, t[0].overdue)
        assertEquals(1, t[1].tasks.size)
    }

    @Test
    fun listsAnExtraTagEvenWhenNothingPendingHasIt() {
        val t = buildTags(emptyList(), 0, "done-only")
        assertEquals(listOf("done-only"), t.map { it.name })
        assertEquals(0, t[0].tasks.size)
        assertEquals(0, t[0].overdue)
    }
}
