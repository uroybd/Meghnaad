package me.utsob.meghnaad.app

import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class CompoundTest {
    private fun split(line: String) = splitChain(line).map { it.join to it.line }

    @Test
    fun isOneStatementWithoutAnOperatorAsTyped() {
        assertEquals(listOf(null to "1 done  outcome:\"a b\""), split("  1 done  outcome:\"a b\" "))
        assertEquals(emptyList(), split(""))
        assertEquals(emptyList(), split(" ; "))
    }

    @Test
    fun splitsOnAndOrAndThenWithOrWithoutSpaces() {
        val chain = listOf(null to "a", Joiner.And to "b", Joiner.Or to "c", Joiner.Then to "d")
        assertEquals(chain, split("a && b || c ; d"))
        assertEquals(chain, split("a&&b||c;d"))
    }

    @Test
    fun leavesASingleAmpersandOrBarAlone() {
        assertEquals(listOf(null to "add R&D | ops"), split("add R&D | ops"))
    }

    @Test
    fun keepsAnOperatorInQuotesOrAfterABackslash() {
        assertEquals(listOf(null to "add 'a && b'"), split("add 'a && b'"))
        assertEquals(listOf(null to "add \"x ; y\"", Joiner.Then to "list"), split("add \"x ; y\" ; list"))
        assertEquals(listOf(null to "add a';'b"), split("add a\\;b"))
        assertEquals(listOf(null to "+a '|''|' +b list"), split("+a \\|\\| +b list"))
    }

    @Test
    fun allowsATrailingSemicolon() {
        assertEquals(listOf(null to "a"), split("a ;"))
        assertEquals(listOf(null to "a", Joiner.And to "b"), split("a && b;"))
    }

    @Test
    fun saysWhatIsMissing() {
        assertEquals("Nothing before ‘&&’.", assertFailsWith<ChainError> { splitChain("&& a") }.message)
        assertEquals("Nothing after ‘&&’.", assertFailsWith<ChainError> { splitChain("a &&") }.message)
        assertEquals("Nothing between ‘&&’ and ‘;’.", assertFailsWith<ChainError> { splitChain("a && ; b") }.message)
        assertFailsWith<ChainError> { splitChain("a ; ; b") }
    }

    private suspend fun go(joins: List<Joiner?>, results: List<Boolean>): Pair<List<Int>, Boolean> {
        val ran = mutableListOf<Int>()
        val ok = runSequence(joins) { i ->
            ran += i
            results[i]
        }
        return ran to ok
    }

    @Test
    fun andRunsTheNextAfterASuccessAndNotAfterAFailure() = runBlocking {
        assertEquals(listOf(0, 1, 2) to true, go(listOf(null, Joiner.And, Joiner.And), listOf(true, true, true)))
        assertEquals(listOf(0, 1) to false, go(listOf(null, Joiner.And, Joiner.And), listOf(true, false, true)))
    }

    @Test
    fun orRunsTheNextAfterAFailureAndNotAfterASuccess() = runBlocking {
        assertEquals(listOf(0, 1) to true, go(listOf(null, Joiner.Or), listOf(false, true)))
        assertEquals(listOf(0) to true, go(listOf(null, Joiner.Or), listOf(true, false)))
    }

    @Test
    fun thenRunsTheNextWhateverHappened() = runBlocking {
        assertEquals(listOf(0, 1, 2) to true, go(listOf(null, Joiner.Then, Joiner.Then), listOf(false, false, true)))
    }

    @Test
    fun goesLeftToRightAndASkippedStatementLeavesTheStatusAsItWas() = runBlocking {
        // (a && b) || c: a fails, b is skipped, c runs.
        assertEquals(listOf(0, 2) to true, go(listOf(null, Joiner.And, Joiner.Or), listOf(false, true, true)))
        // (a || b) && c: a succeeds, b is skipped, c runs.
        assertEquals(listOf(0, 2) to true, go(listOf(null, Joiner.Or, Joiner.And), listOf(true, true, true)))
        // a || b || c: a succeeds, both are skipped.
        assertEquals(listOf(0) to true, go(listOf(null, Joiner.Or, Joiner.Or), listOf(true, true, true)))
    }
}
