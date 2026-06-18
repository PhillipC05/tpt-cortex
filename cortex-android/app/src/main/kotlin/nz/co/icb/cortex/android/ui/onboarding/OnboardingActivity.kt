package nz.co.icb.cortex.android.ui.onboarding

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.viewpager2.widget.ViewPager2
import com.google.android.material.tabs.TabLayout
import com.google.android.material.tabs.TabLayoutMediator
import nz.co.icb.cortex.android.R
import nz.co.icb.cortex.android.databinding.ActivityOnboardingBinding
import nz.co.icb.cortex.android.ui.DashboardActivity

class OnboardingActivity : AppCompatActivity() {

    private lateinit var binding: ActivityOnboardingBinding
    private lateinit var adapter: OnboardingPagerAdapter

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityOnboardingBinding.inflate(layoutInflater)
        setContentView(binding.root)

        adapter = OnboardingPagerAdapter(this)
        binding.viewPager.adapter = adapter

        TabLayoutMediator(binding.tabIndicator, binding.viewPager) { _, _ -> }.attach()

        binding.viewPager.registerOnPageChangeCallback(object : ViewPager2.OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                updateButtons(position)
            }
        })

        binding.btnBack.setOnClickListener {
            val current = binding.viewPager.currentItem
            if (current > 0) binding.viewPager.currentItem = current - 1
        }

        binding.btnNext.setOnClickListener {
            val current = binding.viewPager.currentItem
            if (current < adapter.itemCount - 1) {
                binding.viewPager.currentItem = current + 1
            } else {
                completeOnboarding()
            }
        }

        updateButtons(0)
    }

    private fun updateButtons(position: Int) {
        binding.btnBack.isEnabled = position > 0
        binding.btnNext.text = if (position == adapter.itemCount - 1) {
            getString(R.string.finish)
        } else {
            getString(R.string.next)
        }
    }

    fun completeOnboarding() {
        getSharedPreferences("cortex_prefs", MODE_PRIVATE)
            .edit()
            .putBoolean("onboarding_complete", true)
            .apply()
        startActivity(Intent(this, DashboardActivity::class.java))
        finish()
    }
}
