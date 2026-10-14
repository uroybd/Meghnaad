package me.utsob.meghnaad.app

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.graphics.Color

// The app's colours: the tokens of web/src/app.css, light and dark. The theme follows the system (nothing sets `data-theme`
// on the web, so there is no switch of its own). The few the Material scheme has no name for are in [Extras].

/** What the web's tokens call `--ok`, `--warn` and the three priorities, and the dim text. */
@Immutable
class Extras(
    val ok: Color,
    val warn: Color,
    val priorityHigh: Color,
    val priorityMedium: Color,
    val priorityLow: Color,
    val dim: Color,
    /** The page's own background and text, which colours are judged against. */
    val dark: Boolean,
)

private val LightExtras = Extras(
    ok = Color(0xFF1A7F37),
    warn = Color(0xFF9A6700),
    priorityHigh = Color(0xFFCF222E),
    priorityMedium = Color(0xFF9A6700),
    priorityLow = Color(0xFF1A7F37),
    dim = Color(0xFF75756F),
    dark = false,
)

private val DarkExtras = Extras(
    ok = Color(0xFF56D364),
    warn = Color(0xFFE3B341),
    priorityHigh = Color(0xFFFF7B72),
    priorityMedium = Color(0xFFE3B341),
    priorityLow = Color(0xFF56D364),
    dim = Color(0xFF9A9A90),
    dark = true,
)

private val LightScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF1F6FEB),
    onPrimary = Color(0xFFFFFFFF),
    background = Color(0xFFFBFBFA),
    onBackground = Color(0xFF1C1C1A),
    surface = Color(0xFFFBFBFA),
    onSurface = Color(0xFF1C1C1A),
    surfaceVariant = Color(0xFFF3F3F1),
    onSurfaceVariant = Color(0xFF75756F),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFFFFFFF),
    surfaceContainer = Color(0xFFFFFFFF),
    surfaceContainerHigh = Color(0xFFF3F3F1),
    surfaceContainerHighest = Color(0xFFF3F3F1),
    outline = Color(0xFFE2E2DD),
    outlineVariant = Color(0xFFE2E2DD),
    error = Color(0xFFCF222E),
    onError = Color(0xFFFFFFFF),
)

private val DarkScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFF58A6FF),
    onPrimary = Color(0xFF0B1220),
    background = Color(0xFF14150F),
    onBackground = Color(0xFFE9E9E2),
    surface = Color(0xFF14150F),
    onSurface = Color(0xFFE9E9E2),
    surfaceVariant = Color(0xFF24251E),
    onSurfaceVariant = Color(0xFF9A9A90),
    surfaceContainerLowest = Color(0xFF1C1D17),
    surfaceContainerLow = Color(0xFF1C1D17),
    surfaceContainer = Color(0xFF1C1D17),
    surfaceContainerHigh = Color(0xFF24251E),
    surfaceContainerHighest = Color(0xFF24251E),
    outline = Color(0xFF34352C),
    outlineVariant = Color(0xFF34352C),
    error = Color(0xFFFF7B72),
    onError = Color(0xFF0B1220),
)

val LocalExtras = compositionLocalOf { LightExtras }

/** The app's colours, for the system's light or dark. */
@Composable
fun AppTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalExtras provides if (dark) DarkExtras else LightExtras) {
        MaterialTheme(colorScheme = if (dark) DarkScheme else LightScheme, content = content)
    }
}

val extras: Extras
    @Composable get() = LocalExtras.current
