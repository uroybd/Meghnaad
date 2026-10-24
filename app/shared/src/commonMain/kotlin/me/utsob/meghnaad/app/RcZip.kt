package me.utsob.meghnaad.app

/** The zip of the taskrc folder "Download all" makes: files stored uncompressed, as zip.ts writes them. */
object RcZip {
    private val table = IntArray(256) { n ->
        var c = n
        repeat(8) { c = if (c and 1 != 0) (0xEDB88320.toInt() xor (c ushr 1)) else c ushr 1 }
        c
    }

    fun crc32(bytes: ByteArray): Int {
        var c = -1
        for (b in bytes) c = table[(c xor b.toInt()) and 0xff] xor (c ushr 8)
        return c.inv()
    }

    /** DOS time and date of a moment (UTC), which a zip can't write before 1980. */
    private fun dos(epochSeconds: Long): Pair<Int, Int> {
        val days = Math.floorDiv(epochSeconds, 86400L)
        val secs = Math.floorMod(epochSeconds, 86400L).toInt()
        // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
        val z = days + 719468
        val era = Math.floorDiv(z, 146097L)
        val doe = (z - era * 146097).toInt()
        val yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365
        val doy = doe - (365 * yoe + yoe / 4 - yoe / 100)
        val mp = (5 * doy + 2) / 153
        val day = doy - (153 * mp + 2) / 5 + 1
        val month = if (mp < 10) mp + 3 else mp - 9
        val year = (yoe + era * 400).toInt() + if (month <= 2) 1 else 0
        val time = ((secs / 3600) shl 11) or ((secs % 3600 / 60) shl 5) or (secs % 60 / 2)
        return time to (((maxOf(year, 1980) - 1980) shl 9) or (month shl 5) or day)
    }

    /** The files as a zip, each at its path (`taskrc`, `holidays/en-US.rc`), names read as UTF-8. */
    fun zip(files: List<Pair<String, String>>, epochSeconds: Long): ByteArray {
        val (time, date) = dos(epochSeconds)
        val entries = files.map { (path, text) -> path.encodeToByteArray() to text.encodeToByteArray() }
        val out = ByteArray(entries.sumOf { (n, d) -> 30 + 46 + 2 * n.size + d.size } + 22)
        var at = 0
        fun u16(v: Int) { out[at++] = v.toByte(); out[at++] = (v ushr 8).toByte() }
        fun u32(v: Int) { u16(v and 0xffff); u16(v ushr 16) }
        fun put(b: ByteArray) { b.copyInto(out, at); at += b.size }

        val offsets = ArrayList<Int>()
        for ((name, data) in entries) {
            offsets.add(at)
            u32(0x04034b50); u16(20); u16(0x0800); u16(0); u16(time); u16(date)
            u32(crc32(data)); u32(data.size); u32(data.size); u16(name.size); u16(0)
            put(name); put(data)
        }
        val directory = at
        entries.forEachIndexed { i, (name, data) ->
            u32(0x02014b50); u16(20); u16(20); u16(0x0800); u16(0); u16(time); u16(date)
            u32(crc32(data)); u32(data.size); u32(data.size); u16(name.size)
            u16(0); u16(0); u16(0); u16(0); u32(0); u32(offsets[i])
            put(name)
        }
        val length = at - directory
        u32(0x06054b50); u16(0); u16(0); u16(entries.size); u16(entries.size); u32(length); u32(directory); u16(0)
        return out
    }
}
