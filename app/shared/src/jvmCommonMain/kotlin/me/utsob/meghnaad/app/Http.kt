package me.utsob.meghnaad.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

actual suspend fun httpGet(url: String, limit: Int): HttpAnswer = withContext(Dispatchers.IO) {
    val connection = URL(url).openConnection() as HttpURLConnection
    try {
        connection.connectTimeout = 15_000
        connection.readTimeout = 15_000
        // A redirect is the caller's to follow, so that each address can be checked.
        connection.instanceFollowRedirects = false
        val status = connection.responseCode
        val stream = if (status in 200..299) connection.inputStream else connection.errorStream
        // One byte past the limit is enough to tell the caller it is too long.
        val body = stream?.use { it.readNBytes(limit).toString(Charsets.UTF_8) } ?: ""
        HttpAnswer(status, body, connection.getHeaderField("Location"))
    } finally {
        connection.disconnect()
    }
}
