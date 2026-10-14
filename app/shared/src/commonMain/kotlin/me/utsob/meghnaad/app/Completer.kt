package me.utsob.meghnaad.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

// Tab-completion state shared by the console and the filter bar: the first Tab completes the shared prefix, further Tabs cycle
// through a menu of options (Shift+Tab goes back); web/src/lib/completer.svelte.ts.

class Menu(
    /** The line as it was when the menu opened; every selection is rebuilt from it. */
    val base: String,
    val start: Int,
    val end: Int,
    val options: List<Option>,
    /** -1 = nothing selected yet. */
    sel: Int,
) {
    var sel by mutableStateOf(sel)
}

class Edit(val line: String, val caret: Int)

class Completer {
    var menu by mutableStateOf<Menu?>(null)

    fun close() {
        menu = null
    }

    private fun select(sel: Int): Edit {
        val m = menu!!
        m.sel = sel
        val r = insert(m.base, Completion(m.start, m.end, m.options, ""), m.options[sel].value)
        return Edit(r.line, r.caret)
    }

    /** Handle Tab. Returns the edit to apply, or null if there is nothing to complete. */
    fun tab(line: String, caret: Int, vocab: Vocab, reverse: Boolean = false): Edit? {
        val m = menu
        if (m != null) {
            val n = m.options.size
            return select(if (m.sel < 0) (if (reverse) n - 1 else 0) else (m.sel + (if (reverse) -1 else 1) + n) % n)
        }
        val c = complete(line, caret, vocab)
        if (c.options.isEmpty()) return null
        val r = apply(line, c)
        if (c.options.size == 1) return Edit(r.line, r.caret)
        if (r.progressed) {
            // Completed the shared prefix: show what is left to choose from.
            menu = Menu(r.line, c.start, c.start + c.common.length, c.options, -1)
            return Edit(r.line, r.caret)
        }
        menu = Menu(line, c.start, c.end, c.options, -1)
        return select(if (reverse) c.options.size - 1 else 0)
    }

    /** A tapped option (no keyboard on a phone): select it and accept it. */
    fun pick(index: Int): Edit? {
        val m = menu ?: return null
        if (index < 0 || index >= m.options.size) return null
        val e = select(index)
        menu = null
        return e
    }

    /** Arrow keys while the menu is open. */
    fun move(delta: Int): Edit? {
        val m = menu ?: return null
        val n = m.options.size
        return select(if (m.sel < 0) (if (delta == 1) 0 else n - 1) else (m.sel + delta + n) % n)
    }
}
