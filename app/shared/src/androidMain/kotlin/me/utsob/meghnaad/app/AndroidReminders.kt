package me.utsob.meghnaad.app

import android.Manifest
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build

/**
 * Reminders on a phone: notifications, and an alarm that wakes the app (its [REMINDER_RECEIVER]) at the next moment something is
 * worth saying, so they come with the app closed. [ask] shows the system's permission question (an activity does that); without
 * one the answer is only what is already allowed.
 */
class AndroidReminders(context: Context, private val ask: ((Boolean) -> Unit) -> Unit = { it(false) }) : ReminderHost {
    private val context = context.applicationContext
    private val notifications = this.context.getSystemService(NotificationManager::class.java)
    private val alarms = this.context.getSystemService(AlarmManager::class.java)

    override val allowed: Boolean
        get() = (Build.VERSION.SDK_INT < 33 || context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) &&
            notifications.areNotificationsEnabled()

    override fun requestPermission(done: (Boolean) -> Unit) {
        if (Build.VERSION.SDK_INT < 33 || context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) done(allowed) else ask(done)
    }

    override fun show(notice: Notice) {
        notifications.createNotificationChannel(NotificationChannel(CHANNEL, "Reminders", NotificationManager.IMPORTANCE_HIGH).apply { description = "Due dates and tasks coming off wait" })
        val open = context.packageManager.getLaunchIntentForPackage(context.packageName)?.apply {
            notice.uuid?.let { putExtra(EXTRA_TASK, it) }
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val tap = open?.let { PendingIntent.getActivity(context, notice.key.hashCode(), it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE) }
        @Suppress("DEPRECATION")
        val builder = if (Build.VERSION.SDK_INT >= 26) Notification.Builder(context, CHANNEL) else Notification.Builder(context)
        val built = builder
            .setSmallIcon(icon())
            .setContentTitle(notice.title)
            .setContentText(notice.body)
            .setStyle(Notification.BigTextStyle().bigText(notice.body))
            .setContentIntent(tap)
            .setAutoCancel(true)
            .setCategory(Notification.CATEGORY_REMINDER)
            .build()
        notifications.notify(notice.key.hashCode(), built)
    }

    /** The bell the app carries, or the system's own when it has none. */
    private fun icon(): Int =
        context.resources.getIdentifier("ic_stat_reminder", "drawable", context.packageName).takeIf { it != 0 } ?: android.R.drawable.ic_popup_reminder

    override fun scheduleAt(epochSeconds: Long?) {
        val wake = PendingIntent.getBroadcast(
            context, 0, Intent(ACTION_ALARM).setClassName(context.packageName, REMINDER_RECEIVER),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        if (epochSeconds == null) {
            alarms.cancel(wake)
            return
        }
        val at = epochSeconds * 1000
        // On the minute where the phone allows it; otherwise within a few minutes, which is as good for a reminder.
        try {
            if (Build.VERSION.SDK_INT < 31 || alarms.canScheduleExactAlarms()) alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, wake)
            else alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, wake)
        } catch (_: SecurityException) {
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, wake)
        }
    }

    companion object {
        const val CHANNEL = "reminders"
        const val ACTION_ALARM = "me.utsob.meghnaad.app.REMINDER"

        /** The task a notification opens, as an extra of the intent that starts the app. */
        const val EXTRA_TASK = "me.utsob.meghnaad.app.TASK"
        const val REMINDER_RECEIVER = "me.utsob.meghnaad.app.ReminderReceiver"
    }
}
