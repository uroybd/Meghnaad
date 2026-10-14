package me.utsob.meghnaad.app

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver

// Drawing the user's colours. The engine sends a style per task as palette indexes (from the taskrc's `color.*` rules and its
// theme): 0-15 the basic and bright colours, 16-231 the colour cube, 232-255 the grays. Here they are only a *tint*: a faint
// wash of the colour behind the card, never full-strength text or background, so the text stays the theme's own colour and is
// always readable (the web's contrast guard is not needed). The soft sixteen and the exact cube and gray ramp are the web's
// (web/src/lib/colors.ts), so `color.active=bold on sage` and `rgb500` come out as the colour that was meant.

private val SOFT_LIGHT = longArrayOf(
    0x3a3a36, 0xcf5360, 0x3a8f64, 0xb7791f, 0x3b82c4, 0x9b5fc0, 0x1f9aa5, 0x9aa0a6,
    0x6b7280, 0xe5484d, 0x30a46c, 0xc58b00, 0x2f80ed, 0xb36ae2, 0x12b5c8, 0xb8bec6,
)
private val SOFT_DARK = longArrayOf(
    0x8b8b82, 0xf08a8f, 0x8fd1a5, 0xe8c36a, 0x86b8f0, 0xc7a0ea, 0x7fd6d9, 0xd8dce0,
    0x9aa2ad, 0xff9aa0, 0x9be3b0, 0xf3d98a, 0xa0c8ff, 0xdab6f5, 0x95e6ea, 0xf2f4f6,
)
private val CUBE = intArrayOf(0, 95, 135, 175, 215, 255)

/** The colour of palette index [i], as the app draws it. */
fun paletteColor(i: Int, dark: Boolean): Color {
    if (i < 16) return Color(0xFF000000 or (if (dark) SOFT_DARK else SOFT_LIGHT)[i.coerceAtLeast(0)])
    if (i < 232) {
        val n = i - 16
        return Color(CUBE[n / 36], CUBE[n / 6 % 6], CUBE[n % 6])
    }
    val v = (8 + 10 * (i - 232)).coerceIn(0, 255)
    return Color(v, v, v)
}

/**
 * The colour a style tints with: its background if it has one, else its foreground, after `inverse` swaps them. Null when the
 * style says nothing (no rule applies, or colour is off: then no style arrives at all).
 */
fun Resolved.tintSource(dark: Boolean): Color? {
    val fgIndex = if (inverse) bg else fg
    val bgIndex = if (inverse) fg else bg
    return (bgIndex ?: fgIndex)?.let { paletteColor(it, dark) }
}

/** The wash of a style over [surface]: a little stronger on a dark page, where the same share shows less. */
fun Resolved?.tint(dark: Boolean, surface: Color): Color? {
    val source = this?.tintSource(dark) ?: return null
    return source.copy(alpha = if (dark) 0.18f else 0.13f).compositeOver(surface)
}
