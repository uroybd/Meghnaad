package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

// The menuKey and setupQuestion cases of web/src/lib/tasksh.test.ts that TaskshTest folds into fewer checks.
class TaskshMenuTest {
    @Test
    fun theMenuKnowsItsChoicesAndEnterIsTheUsualOne() {
        assertEquals(
            listOf(ReviewKey.Reviewed, ReviewKey.Reviewed, ReviewKey.Skip, ReviewKey.Edit, ReviewKey.Modify, ReviewKey.Complete, ReviewKey.Delete, ReviewKey.Quit),
            listOf("", "r", "s", "e", "m", "c", "d", "q").map { menuKey(it) },
        )
        assertEquals(ReviewKey.Skip, menuKey("  S "))
    }

    @Test
    fun theMenuDoesNotKnowAnyOtherWord() {
        for (w in listOf("x", "review", "yes", "ss", "1")) assertNull(menuKey(w), w)
    }

    @Test
    fun theSetupQuestionsAreTaskshOwnWhenSomethingIsMissing() {
        val s = taskshStatus(null)
        assertEquals("Tasksh needs to define a 'reviewed' UDA of type 'date' for all tasks.  Ok to proceed?", setupQuestion(SetupPart.Uda, s))
        assertEquals("Tasksh needs to define a '_reviewed' report to identify tasks needing review.  Ok to proceed?", setupQuestion(SetupPart.Report, s))
    }
}
