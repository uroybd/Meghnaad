package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Ported from web/src/lib/urgency.test.ts.
class UrgencyTest {
    private val defaults = mapOf("urgency.due.coefficient" to 12.0, "urgency.user.tag.next.coefficient" to 15.0)

    @Test
    fun namesBuiltInTermsAndTheAgeCap() {
        assertEquals("Due date", describeUrgency("urgency.due.coefficient").label)
        assertEquals("Age cap (days)", describeUrgency(AGE_MAX).label)
        assertEquals(UrgencyKind.Term, describeUrgency(AGE_MAX).kind)
    }

    @Test
    fun saysAProjectCoefficientReachesItsSubProjects() {
        val d = describeUrgency("urgency.user.project.Home.Kitchen.coefficient")
        assertEquals(UrgencyKind.Project, d.kind)
        assertEquals("Home.Kitchen", d.name)
        assertTrue("Home.Kitchen.*" in d.hint)
    }

    @Test
    fun readsTagsKeywordsAndUdaValues() {
        assertEquals("next", describeUrgency("urgency.user.tag.next.coefficient").name)
        assertEquals(UrgencyKind.Keyword, describeUrgency("urgency.user.keyword.call.coefficient").kind)
        assertEquals("priority = H", describeUrgency("urgency.uda.priority.H.coefficient").label)
        assertEquals("estimate has a value", describeUrgency("urgency.uda.estimate.coefficient").label)
    }

    @Test
    fun buildsTheKeysTaskwarriorReads() {
        assertEquals("urgency.user.project.Home.coefficient", buildUrgencyKey(UrgencyKind.Project, "Home"))
        assertEquals("urgency.user.tag.next.coefficient", buildUrgencyKey(UrgencyKind.Tag, "next"))
        assertEquals("urgency.user.keyword.call.coefficient", buildUrgencyKey(UrgencyKind.Keyword, "call"))
        assertEquals("urgency.uda.estimate.huge.coefficient", buildUrgencyKey(UrgencyKind.Uda, "estimate.huge"))
    }

    @Test
    fun refusesNamesThatCouldNotSurviveATaskrc() {
        for (bad in listOf("", "a b", "a=b", "a#b", "x.coefficient")) assertFalse(validUrgencyName(bad), bad)
        assertTrue(validUrgencyName("Home.Kitchen"))
        assertTrue(validUda("estimate"))
        assertTrue(validUda("estimate.huge"))
        assertFalse(validUda("bad-name"))
    }

    @Test
    fun listsBuiltInTermsBeforeCustomSettingsInTaskwarriorOrder() {
        val keys = listOf(
            "urgency.user.tag.b.coefficient", "urgency.user.project.A.coefficient", "urgency.age.coefficient", AGE_MAX, "urgency.project.coefficient",
        )
        assertEquals(
            listOf("urgency.project.coefficient", "urgency.age.coefficient", AGE_MAX, "urgency.user.project.A.coefficient", "urgency.user.tag.b.coefficient"),
            sortUrgencyKeys(keys),
        )
    }

    @Test
    fun fallsBackToTheBuiltInValue() {
        assertEquals(12.0, effectiveUrgency("urgency.due.coefficient", emptyMap(), defaults))
        assertEquals(3.0, effectiveUrgency("urgency.due.coefficient", mapOf("urgency.due.coefficient" to 3.0), defaults))
        assertNull(effectiveUrgency("urgency.user.tag.x.coefficient", emptyMap(), defaults))
    }

    @Test
    fun dropsAnOverrideThatEqualsTheDefaultAndKeepsOthers() {
        val start = mapOf("urgency.due.coefficient" to 3.0)
        assertEquals(emptyMap(), withUrgencyValue(start, defaults, "urgency.due.coefficient", 12.0))
        assertEquals(mapOf("urgency.due.coefficient" to 0.0), withUrgencyValue(start, defaults, "urgency.due.coefficient", 0.0))
        assertEquals(mapOf("urgency.user.tag.x.coefficient" to 2.0), withUrgencyValue(emptyMap(), defaults, "urgency.user.tag.x.coefficient", 2.0))
        assertEquals(mapOf("urgency.due.coefficient" to 3.0), start)
    }
}
