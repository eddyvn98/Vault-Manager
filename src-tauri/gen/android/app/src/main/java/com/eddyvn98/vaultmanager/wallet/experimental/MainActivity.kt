package com.eddyvn98.vaultmanager.wallet.experimental

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Prevent screenshots, screen recordings, and app switcher leak of wallet keys
    window.setFlags(
      WindowManager.LayoutParams.FLAG_SECURE,
      WindowManager.LayoutParams.FLAG_SECURE
    )
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
