package me.utsob.meghnaad.app

/** What a cell needs to know of the settings, for the report [name] (its own `dateformat` first). */
fun Store.cellCtx(name: String): Ctx {
    val settings = config?.config?.settings ?: emptyMap()
    return Ctx(
        now = now,
        tz = utcOffset,
        offsetAt = ::offsetAt,
        udas = config?.config?.udas ?: emptyMap(),
        reportDates = formatFor(FormatKind.Report, settings, config?.config?.reports?.get(name)?.dateformat),
        annotationDates = formatFor(FormatKind.Annotation, settings),
        recurIndicator = settings["recurrence.indicator"]?.takeIf { it.isNotEmpty() },
        indicators = Indicators(
            active = settings["active.indicator"],
            tag = settings["tag.indicator"],
            dependency = settings["dependency.indicator"],
        ),
        journal = config?.journal,
    )
}
