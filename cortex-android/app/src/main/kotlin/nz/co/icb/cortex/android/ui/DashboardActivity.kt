package nz.co.icb.cortex.android.ui

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Bundle
import android.view.WindowManager
import androidx.appcompat.app.AppCompatActivity
import androidx.localbroadcastmanager.content.LocalBroadcastManager
import androidx.recyclerview.widget.LinearLayoutManager
import nz.co.icb.cortex.android.R
import nz.co.icb.cortex.android.databinding.ActivityDashboardBinding
import nz.co.icb.cortex.android.service.CortexForegroundService

class DashboardActivity : AppCompatActivity() {

    private lateinit var binding: ActivityDashboardBinding
    private lateinit var logAdapter: LogAdapter
    private val logs = mutableListOf<String>()
    private var serviceRunning = false

    private val statusReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                CortexForegroundService.ACTION_STATUS_UPDATE -> {
                    val running = intent.getBooleanExtra(CortexForegroundService.EXTRA_RUNNING, false)
                    val log = intent.getStringExtra(CortexForegroundService.EXTRA_LOG)
                    serviceRunning = running
                    updateStatusUI()
                    log?.let { addLog(it) }
                }
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityDashboardBinding.inflate(layoutInflater)
        setContentView(binding.root)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

        logAdapter = LogAdapter(logs)
        binding.rvLogs.apply {
            adapter = logAdapter
            layoutManager = LinearLayoutManager(this@DashboardActivity).apply {
                stackFromEnd = true
            }
        }

        binding.btnToggleService.setOnClickListener {
            if (serviceRunning) {
                stopCortexService()
            } else {
                startCortexService()
            }
        }

        updateStatusUI()
    }

    override fun onResume() {
        super.onResume()
        val filter = IntentFilter(CortexForegroundService.ACTION_STATUS_UPDATE)
        LocalBroadcastManager.getInstance(this).registerReceiver(statusReceiver, filter)
    }

    override fun onPause() {
        super.onPause()
        LocalBroadcastManager.getInstance(this).unregisterReceiver(statusReceiver)
    }

    private fun startCortexService() {
        val intent = Intent(this, CortexForegroundService::class.java).apply {
            action = CortexForegroundService.ACTION_START_SERVICE
        }
        startForegroundService(intent)
        serviceRunning = true
        updateStatusUI()
        addLog("Starting Cortex service...")
    }

    private fun stopCortexService() {
        val intent = Intent(this, CortexForegroundService::class.java).apply {
            action = CortexForegroundService.ACTION_STOP_SERVICE
        }
        startService(intent)
        serviceRunning = false
        updateStatusUI()
        addLog("Stopping Cortex service...")
    }

    private fun updateStatusUI() {
        if (serviceRunning) {
            binding.tvServiceStatus.text = getString(R.string.status_running)
            binding.tvServiceStatus.setTextColor(getColor(R.color.cortex_green))
            binding.btnToggleService.text = getString(R.string.stop_service)
        } else {
            binding.tvServiceStatus.text = getString(R.string.status_stopped)
            binding.tvServiceStatus.setTextColor(getColor(android.R.color.holo_red_light))
            binding.btnToggleService.text = getString(R.string.start_service)
        }
    }

    private fun addLog(message: String) {
        logs.add(message)
        if (logs.size > 50) logs.removeAt(0)
        logAdapter.notifyDataSetChanged()
        binding.rvLogs.scrollToPosition(logs.size - 1)
    }
}
