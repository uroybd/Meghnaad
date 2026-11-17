package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class HookSecretsTest {
    @Test
    fun theListHasNamesAndHostsAndReadsTheAnswerBeforeHosts() {
        val now = """{"secrets":[{"name":"jira","hosts":["acme.atlassian.net"]},{"name":"old","hosts":[]}],"names":["jira","old"]}"""
        assertEquals(listOf(SecretInfo("jira", listOf("acme.atlassian.net")), SecretInfo("old", emptyList())), secretsFromJson(now))
        assertEquals(listOf(SecretInfo("jira", emptyList())), secretsFromJson("""{"names":["jira"]}"""))
    }

    @Test
    fun aSecretIsSentWithItsHosts() {
        assertEquals(listOf("hooks.slack.com", "*.slack.com"), hostsFromText(" hooks.slack.com, *.slack.com ,"))
        val body = secretBody("tok \"1\"", listOf("a.example"))
        assertTrue("\"value\":\"tok \\\"1\\\"\"" in body && "\"hosts\":[\"a.example\"]" in body, body)
    }
}
