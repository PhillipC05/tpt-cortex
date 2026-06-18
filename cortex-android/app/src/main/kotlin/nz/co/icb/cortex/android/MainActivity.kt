package nz.co.icb.cortex.android

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import nz.co.icb.cortex.android.ui.DashboardActivity
import nz.co.icb.cortex.android.ui.onboarding.OnboardingActivity

class MainActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = getSharedPreferences("cortex_prefs", MODE_PRIVATE)
        val onboardingComplete = prefs.getBoolean("onboarding_complete", false)

        if (onboardingComplete) {
            startActivity(Intent(this, DashboardActivity::class.java))
        } else {
            startActivity(Intent(this, OnboardingActivity::class.java))
        }
        finish()
    }
}
