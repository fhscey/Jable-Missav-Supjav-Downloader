// 主要用于页面访问和解析，绕开不必要的检查

use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::Mutex;
use url::Url;

use crate::network::error::NetworkError;

const CF_WINDOW: &str = "cf-window";

/// 前端上报的数据结构
#[derive(Debug, Deserialize, Clone)]
pub struct PageReadyPayload {
    pub generation: u64,
    pub url: String,
    pub html: String,
    #[serde(default)]
    pub cookie: Option<String>,
    #[serde(default)]
    pub ua: Option<String>,
}

/// Webview 页面抓取完整上下文（包含 DOM、就绪后的真实 URL、站点 Origin 及环境信息）
#[derive(Debug, Clone)]
pub struct FetchPageResult {
    pub html: String,
    pub final_url: String, // 指的是 redirect 之后的最终 url
    pub origin: String,
    pub cookie: Option<String>,
    pub user_agent: Option<String>,
}

/// 检查当前页面是否处于 CF 验证拦截状态
pub fn is_cf_challenge(html: &str) -> bool {
    const CHALLENGE_KEYWORDS: &[&str] = &[
        "<title>just a moment",
        "<title>attention required",
        "<title>please wait...",
        "<title>verify you are human",
        r#"id="challenge-form""#,
        r#"id="challenge-running""#,
        r#"id="challenge-stage""#,
        r#"id="cf-challenge-running""#,
        r#"class="cf-browser-verification""#,
        r#"id="challenge-error-title""#,
        r#"id="challenge-body-text""#,
    ];
    let lower = html.to_lowercase();
    CHALLENGE_KEYWORDS.iter().any(|&kw| lower.contains(kw))
}

/// Webview 抓取引擎（归属于 network 模块）：管理后台 Webview 并根据 Provider 传入的判定条件即时截断返回 HTML
#[derive(Clone)]
pub struct WebviewEngine {
    app: AppHandle,
    current_generation: Arc<AtomicU64>,
    lock: Arc<Mutex<()>>,
}

impl WebviewEngine {
    pub fn new(app: &AppHandle) -> Self {
        Self {
            app: app.clone(),
            current_generation: Arc::new(AtomicU64::new(0)),
            lock: Arc::new(Mutex::new(())),
        }
    }

    /// 获取或创建单例后台 Webview 窗口
    fn get_or_create_window(&self, url: &Url) -> Result<WebviewWindow, NetworkError> {
        if let Some(win) = self.app.get_webview_window(CF_WINDOW) {
            log::info!(
                "[WebviewEngine] 复用已有 Webview 窗口，重置并导航至: {}",
                url
            );
            // 确保窗口在开始加载时处于隐藏状态
            let _ = win.hide();
            // 关键：在发起新导航前，先标记上一页为失效 (stale) 并清空 DOM 内容，
            // 同时维持深色背景与 colorScheme，避免白屏闪烁与旧页面残留 DOM 误判
            let _ = win.eval(
                "window.__avdl_stale = true; try { document.documentElement.style.backgroundColor = '#121214'; document.documentElement.style.colorScheme = 'dark'; document.documentElement.innerHTML = ''; } catch(e) {}",
            );
            let _ = win.navigate(url.clone());
            return Ok(win);
        }

        log::info!("[WebviewEngine] 创建新 Webview 窗口，初始地址: {}", url);
        let win =
            WebviewWindowBuilder::new(&self.app, CF_WINDOW, WebviewUrl::External(url.clone()))
                .title("AVDL 网页加载与验证")
                .inner_size(680.0, 580.0)
                .center()
                .visible(false)
                .theme(Some(tauri::Theme::Dark))
                .background_color(tauri::window::Color(18, 18, 20, 255))
                .initialization_script(
                    r#"
                    (function() {
                        const style = document.createElement('style');
                        style.id = '__avdl_dark_theme';
                        style.textContent = `
                            :root {
                                color-scheme: dark !important;
                            }
                            html, body {
                                background-color: #121214 !important;
                                color: #e4e4e7 !important;
                            }
                        `;
                        const apply = () => {
                            if (document.head && !document.getElementById('__avdl_dark_theme')) {
                                document.head.appendChild(style);
                            }
                        };
                        apply();
                        document.addEventListener('DOMContentLoaded', apply);
                    })();
                    "#,
                )
                .build()
                .map_err(|e| NetworkError::Undefined(format!("创建 Webview 窗口失败: {}", e)))?;

        let win_clone = win.clone();
        win.on_window_event(move |e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = win_clone.hide();
            }
        });

        Ok(win)
    }

    /// 根据传入的就绪条件加载页面，目标就绪即返回完整上下文（DOM、实际跳转 URL、站点 Origin 及网络环境）
    pub async fn fetch_page(
        &self,
        target_url: &str,
        ready_condition: &str,
    ) -> Result<FetchPageResult, NetworkError> {
        let _guard = self.lock.lock().await;

        let parsed_url = Url::parse(target_url)
            .map_err(|e| NetworkError::InvalidUrl(format!("{}: {}", target_url, e)))?;

        // 自增 generation，防止并发竞争与旧请求的脏数据干扰
        let this_generation = self.current_generation.fetch_add(1, Ordering::SeqCst) + 1;

        log::info!(
            "[WebviewEngine] 开始加载页面 [gen={}]: url={}, 条件: {}",
            this_generation,
            target_url,
            ready_condition
        );

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PageReadyPayload>();
        let listener_id = self.app.listen("page-ready", move |e| {
            if let Ok(payload) = serde_json::from_str::<PageReadyPayload>(e.payload()) {
                let _ = tx.send(payload);
            }
        });

        let window = match self.get_or_create_window(&parsed_url) {
            Ok(w) => w,
            Err(e) => {
                self.app.unlisten(listener_id);
                return Err(e);
            }
        };

        let condition = if ready_condition.trim().is_empty() {
            "document.readyState === 'complete'"
        } else {
            ready_condition
        };

        // 注入的探测脚本：优先检测 CF 盾拦截；若非 CF 则检测站点目标元素就绪
        let js_code = format!(
            r#"
            (function() {{
                try {{
                    // 1. 若当前页面上下文已被标记为过期/失效，直接跳过
                    if (window.__avdl_stale) {{
                        return;
                    }}

                    // 2. 若 DOM 树未就绪或处于空白重置态，直接跳过
                    if (!document.documentElement || !document.body || document.body.innerHTML.trim() === '') {{
                        return;
                    }}

                    const htmlLower = document.documentElement ? document.documentElement.outerHTML.toLowerCase() : '';
                    const titleLower = document.title ? document.title.toLowerCase() : '';
                    const isCf = titleLower.includes('just a moment') ||
                                 titleLower.includes('attention required') ||
                                 titleLower.includes('please wait') ||
                                 titleLower.includes('verify you are human') ||
                                 htmlLower.includes('id="challenge-form"') ||
                                 htmlLower.includes('id="challenge-running"') ||
                                 htmlLower.includes('id="challenge-stage"') ||
                                 htmlLower.includes('class="cf-browser-verification"') ||
                                 htmlLower.includes('challenges.cloudflare.com');

                    // 3. 如果处于 CF 拦截状态，立即上报以唤起验证窗口
                    if (isCf && window.__TAURI_INTERNALS__?.invoke) {{
                        window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {{
                            event: 'page-ready',
                            payload: {{
                                generation: {this_generation},
                                url: window.location.href,
                                html: document.documentElement ? document.documentElement.outerHTML : ''
                            }}
                        }});
                        return;
                    }}

                    // 4. 正常页面状态下，判断 Provider 定义的目标元素是否就绪
                    const isReady = Boolean({condition});
                    if (isReady && window.__TAURI_INTERNALS__?.invoke) {{
                        const html = document.documentElement ? document.documentElement.outerHTML : '';
                        if (html) {{
                            // 注意：上报后不执行 window.stop()，以免意外中止网络导航
                            window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {{
                                event: 'page-ready',
                                payload: {{
                                    generation: {this_generation},
                                    url: window.location.href,
                                    html: html,
                                    cookie: document.cookie || '',
                                    ua: navigator.userAgent || ''
                                }}
                            }});
                        }}
                    }}
                }} catch(e) {{}}
            }})();
            "#
        );

        let mut had_challenge = false;
        let total_start_time = tokio::time::Instant::now();
        let mut silent_start_time = tokio::time::Instant::now();
        let silent_timeout = Duration::from_secs(60);
        let check_interval = Duration::from_millis(500);

        let result = loop {
            let is_visible = window.is_visible().unwrap_or(false);

            // 1. 若曾唤起过交互窗口，但当前窗口不可见，说明用户主动关闭了窗口取消验证
            if had_challenge && !is_visible {
                log::warn!("[WebviewEngine] 用户手动关闭了验证窗口");
                break Err(NetworkError::Undefined("用户取消了验证".to_string()));
            }

            // 2. 状态化超时处理：
            // - 窗口隐藏（后台静默加载阶段）：受 silent_timeout 超时约束
            // - 窗口可见（用户交互验证阶段）：不设超时限制，让用户从容交互或等待；同时持续刷新静默计时起点
            if !is_visible {
                let elapsed = silent_start_time.elapsed();
                if elapsed > silent_timeout {
                    let _ = window.hide();
                    log::error!(
                        "[WebviewEngine] 静默加载超时 ({}s)，未能在规定时间内满足就绪条件: url={}, 条件: {}",
                        silent_timeout.as_secs(),
                        target_url,
                        condition
                    );
                    break Err(NetworkError::Undefined(format!(
                        "加载超时 ({}s)，目标地址: {}",
                        silent_timeout.as_secs(),
                        target_url
                    )));
                }
            } else {
                silent_start_time = tokio::time::Instant::now();
            }

            // 周期性注入执行就绪检测代码
            let _ = window.eval(&js_code);

            tokio::select! {
                Some(payload) = rx.recv() => {
                    // 比对 generation，丢弃不匹配或过期的事件
                    if payload.generation != this_generation {
                        log::debug!(
                            "[WebviewEngine] 丢弃过期事件: payload_gen={}, current_gen={}",
                            payload.generation,
                            this_generation
                        );
                        continue;
                    }

                    if payload.html.trim().is_empty() {
                        continue;
                    }

                    if is_cf_challenge(&payload.html) {
                        if !had_challenge {
                            log::warn!("[WebviewEngine] 遇到拦截，唤出窗口等待用户验证: {}", payload.url);
                            had_challenge = true;
                            let _ = window.show();
                            let _ = window.set_focus();
                        }

                        // 检查用户是否手动关闭了验证窗口
                        if !window.is_visible().unwrap_or(true) {
                            log::warn!("[WebviewEngine] 用户手动关闭了验证窗口");
                            break Err(NetworkError::Undefined("用户取消了验证".to_string()));
                        }

                        // 等待用户在窗口中完成验证，休眠避免高频空转
                        tokio::time::sleep(check_interval).await;
                    } else {
                        if had_challenge {
                            log::info!("[WebviewEngine] 验证已完成，隐藏窗口");
                        }
                        let _ = window.hide();
                        log::info!(
                            "[WebviewEngine] 目标元素就绪 [gen={}]，成功返回 HTML ({} bytes，耗时 {:.2}s)",
                            this_generation,
                            payload.html.len(),
                            total_start_time.elapsed().as_secs_f32()
                        );
                        let origin = if let Ok(u) = Url::parse(&payload.url) {
                            format!("{}/", u.origin().ascii_serialization().trim_end_matches('/'))
                        } else {
                            format!("{}/", payload.url.trim_end_matches('/'))
                        };

                        break Ok(FetchPageResult {
                            html: payload.html,
                            final_url: payload.url,
                            origin,
                            cookie: payload.cookie,
                            user_agent: payload.ua,
                        });
                    }
                }
                _ = tokio::time::sleep(check_interval) => {
                    if had_challenge && !window.is_visible().unwrap_or(true) {
                        log::warn!("[WebviewEngine] 用户手动关闭了验证窗口");
                        break Err(NetworkError::Undefined("用户取消了验证".to_string()));
                    }
                }
            }
        };

        self.app.unlisten(listener_id);
        result
    }

    /// 根据传入的就绪条件加载页面，目标就绪即返回 HTML 字符串
    pub async fn fetch_html(
        &self,
        target_url: &str,
        ready_condition: &str,
    ) -> Result<String, NetworkError> {
        self.fetch_page(target_url, ready_condition)
            .await
            .map(|r| r.html)
    }
}
