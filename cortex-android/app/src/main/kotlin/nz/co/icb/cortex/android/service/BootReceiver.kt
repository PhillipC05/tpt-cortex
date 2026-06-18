package nz.co.icb.cortex.android.service

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            val serviceIntent = Intent(context, CortexForegroundService::class.java).apply {
                action = CortexForegroundService.ACTION_START_SERVICE
            }
            context.startForegroundService(serviceIntent)
            CortexWorker.schedule(context)
        }
    }
}
