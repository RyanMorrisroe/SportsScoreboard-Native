package com.ryan.sportsscoreboard

import android.content.Context
import android.graphics.Color
import android.os.Bundle
import android.view.View
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  companion object {
    init {
      System.loadLibrary("app_lib")
    }

    @JvmStatic external fun initializeRustlsPlatformVerifier(context: Context)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    initializeRustlsPlatformVerifier(applicationContext)
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)

    val content = findViewById<View>(android.R.id.content)
    content.setBackgroundColor(Color.rgb(18, 18, 18))
    val basePadding = intArrayOf(
      content.paddingLeft,
      content.paddingTop,
      content.paddingRight,
      content.paddingBottom,
    )
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val safeInsets = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
      )
      view.setPadding(
        basePadding[0] + safeInsets.left,
        basePadding[1] + safeInsets.top,
        basePadding[2] + safeInsets.right,
        basePadding[3] + safeInsets.bottom,
      )
      insets
    }
    ViewCompat.requestApplyInsets(content)
  }
}
