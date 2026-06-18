package nz.co.icb.cortex.android.ui.onboarding

import androidx.fragment.app.Fragment
import androidx.fragment.app.FragmentActivity
import androidx.viewpager2.adapter.FragmentStateAdapter

class OnboardingPagerAdapter(activity: FragmentActivity) : FragmentStateAdapter(activity) {

    override fun getItemCount(): Int = 3

    override fun createFragment(position: Int): Fragment = when (position) {
        0 -> WelcomeFragment()
        1 -> BatteryFragment()
        2 -> PermissionFragment()
        else -> WelcomeFragment()
    }
}
