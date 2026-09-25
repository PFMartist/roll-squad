package com.sdgqy.rollsquad

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  // 前端是按 1280 宽设计的（六列两行的编队格、右下角操作条）。
  // 手机横屏的 CSS 视口只有 600~800 宽，会掉进窄屏分支被排成四列 ——
  // 这里打开 useWideViewPort，让 index.html 里那段 width=1280 真正生效：
  // 所有机型都用同一套版面，由系统按屏幕宽度整体缩放。
  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    webView.settings.useWideViewPort = true
    webView.settings.loadWithOverviewMode = true
  }
}
