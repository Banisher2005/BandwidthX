use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedTestProgress {
    pub is_running: bool,
    pub phase: String, // "idle" | "latency" | "download" | "upload" | "completed" | "cancelled" | "error"
    pub progress_percent: u32,
    pub ping_ms: f64,
    pub jitter_ms: f64,
    pub download_speed_bps: u64,
    pub upload_speed_bps: u64,
    pub current_throughput_bps: u64,
    pub bytes_transferred: u64,
    pub server_name: String,
    pub error_message: Option<String>,
    pub timestamp: Option<String>,
}

impl Default for SpeedTestProgress {
    fn default() -> Self {
        Self {
            is_running: false,
            phase: "idle".to_string(),
            progress_percent: 0,
            ping_ms: 0.0,
            jitter_ms: 0.0,
            download_speed_bps: 0,
            upload_speed_bps: 0,
            current_throughput_bps: 0,
            bytes_transferred: 0,
            server_name: "Cloudflare Speed (Default)".to_string(),
            error_message: None,
            timestamp: None,
        }
    }
}

pub struct SpeedTestEngine {
    client: reqwest::Client,
    progress: Arc<RwLock<SpeedTestProgress>>,
    cancel_flag: Arc<AtomicBool>,
}

impl SpeedTestEngine {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(4)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            progress: Arc::new(RwLock::new(SpeedTestProgress::default())),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn get_progress(&self) -> SpeedTestProgress {
        self.progress.read().clone()
    }

    pub fn cancel(&self) -> bool {
        self.cancel_flag.store(true, Ordering::SeqCst);
        let mut p = self.progress.write();
        p.is_running = false;
        p.phase = "cancelled".to_string();
        true
    }

    pub async fn run_test(&self, custom_endpoint: Option<String>) -> Result<SpeedTestProgress, String> {
        self.cancel_flag.store(false, Ordering::SeqCst);

        let endpoint_base = custom_endpoint
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| "https://speed.cloudflare.com".to_string());

        let server_name = if endpoint_base.contains("cloudflare") {
            "Cloudflare Edge CDN (Privacy-Preserving)".to_string()
        } else {
            format!("Custom Server ({})", endpoint_base)
        };

        {
            let mut p = self.progress.write();
            *p = SpeedTestProgress {
                is_running: true,
                phase: "latency".to_string(),
                progress_percent: 5,
                server_name: server_name.clone(),
                timestamp: Some(chrono::Local::now().to_rfc3339()),
                ..Default::default()
            };
        }

        // ---------------------------------------------------------------------
        // 1. LATENCY & JITTER PHASE (10 Samples)
        // ---------------------------------------------------------------------
        let ping_url = format!("{}/__down?bytes=0", endpoint_base);
        let mut samples = Vec::with_capacity(10);

        for i in 0..10 {
            if self.cancel_flag.load(Ordering::SeqCst) {
                return Err("Test cancelled by user".to_string());
            }

            let start = Instant::now();
            let res = self.client.get(&ping_url).send().await;
            match res {
                Ok(resp) => {
                    let _ = resp.bytes().await;
                    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                    samples.push(elapsed_ms);
                }
                Err(e) => {
                    if i == 0 {
                        let err_msg = format!("Network unreachable or invalid test endpoint: {}", e);
                        let mut p = self.progress.write();
                        p.is_running = false;
                        p.phase = "error".to_string();
                        p.error_message = Some(err_msg.clone());
                        return Err(err_msg);
                    }
                }
            }

            {
                let mut p = self.progress.write();
                p.progress_percent = 5 + ((i + 1) * 2) as u32; // up to 25%
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if samples.is_empty() {
            let mut p = self.progress.write();
            p.is_running = false;
            p.phase = "error".to_string();
            p.error_message = Some("Failed to obtain latency measurements".to_string());
            return Err("Failed to obtain latency measurements".to_string());
        }

        // Compute mean latency and jitter (Mean Absolute Deviation of consecutive diffs)
        let sum_lat: f64 = samples.iter().sum();
        let avg_latency = sum_lat / (samples.len() as f64);

        let mut jitter = 0.0;
        if samples.len() > 1 {
            let mut diff_sum = 0.0;
            for w in samples.windows(2) {
                diff_sum += (w[1] - w[0]).abs();
            }
            jitter = diff_sum / ((samples.len() - 1) as f64);
        }

        {
            let mut p = self.progress.write();
            p.ping_ms = (avg_latency * 10.0).round() / 10.0;
            p.jitter_ms = (jitter * 10.0).round() / 10.0;
            p.phase = "download".to_string();
            p.progress_percent = 30;
        }

        // ---------------------------------------------------------------------
        // 2. DOWNLOAD THROUGHPUT PHASE (Transferring Actual Bytes)
        // ---------------------------------------------------------------------
        // Transfer 15 MB chunk
        let download_url = format!("{}/__down?bytes=15000000", endpoint_base);
        let dl_start = Instant::now();

        let dl_res = self.client.get(&download_url).send().await;
        let mut total_dl_bytes = 0u64;

        match dl_res {
            Ok(mut resp) => {
                let mut last_progress_tick = Instant::now();
                while let Ok(Some(chunk)) = resp.chunk().await {
                    if self.cancel_flag.load(Ordering::SeqCst) {
                        return Err("Test cancelled by user".to_string());
                    }

                    total_dl_bytes += chunk.len() as u64;

                    // Update live throughput every 150ms
                    if last_progress_tick.elapsed().as_millis() > 150 {
                        let cur_elapsed = dl_start.elapsed().as_secs_f64();
                        if cur_elapsed > 0.05 {
                            let cur_bps = (total_dl_bytes as f64 / cur_elapsed) as u64;
                            let pct = 30 + ((total_dl_bytes as f64 / 15000000.0) * 35.0).min(35.0) as u32;
                            let mut p = self.progress.write();
                            p.current_throughput_bps = cur_bps;
                            p.download_speed_bps = cur_bps;
                            p.bytes_transferred = total_dl_bytes;
                            p.progress_percent = pct;
                        }
                        last_progress_tick = Instant::now();
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("Download test failed: {}", e);
                let mut p = self.progress.write();
                p.is_running = false;
                p.phase = "error".to_string();
                p.error_message = Some(err_msg.clone());
                return Err(err_msg);
            }
        }

        let dl_duration = dl_start.elapsed().as_secs_f64();
        let final_dl_bps = if dl_duration > 0.1 && total_dl_bytes > 0 {
            (total_dl_bytes as f64 / dl_duration) as u64
        } else {
            0
        };

        {
            let mut p = self.progress.write();
            p.download_speed_bps = final_dl_bps;
            p.current_throughput_bps = 0;
            p.phase = "upload".to_string();
            p.progress_percent = 65;
        }

        // ---------------------------------------------------------------------
        // 3. UPLOAD THROUGHPUT PHASE (Transferring Actual Bytes)
        // ---------------------------------------------------------------------
        // Transfer 5 MB buffer
        let upload_url = format!("{}/__up", endpoint_base);
        let upload_payload = vec![0x58u8; 5 * 1024 * 1024]; // 5 MB
        let ul_start = Instant::now();

        let ul_res = self.client.post(&upload_url)
            .body(upload_payload)
            .send()
            .await;

        let mut final_ul_bps = 0u64;
        match ul_res {
            Ok(resp) => {
                let _ = resp.bytes().await;
                let ul_duration = ul_start.elapsed().as_secs_f64();
                if ul_duration > 0.1 {
                    final_ul_bps = ((5 * 1024 * 1024) as f64 / ul_duration) as u64;
                }
            }
            Err(e) => {
                // If upload endpoint is unsupported by custom server, record 0 rather than aborting overall test
                eprintln!("[SpeedTest] Upload test warning: {}", e);
            }
        }

        // ---------------------------------------------------------------------
        // 4. COMPLETION
        // ---------------------------------------------------------------------
        let mut final_p = self.progress.write();
        final_p.is_running = false;
        final_p.phase = "completed".to_string();
        final_p.progress_percent = 100;
        final_p.upload_speed_bps = final_ul_bps;
        final_p.current_throughput_bps = 0;
        final_p.bytes_transferred = total_dl_bytes + (5 * 1024 * 1024);

        Ok(final_p.clone())
    }
}
