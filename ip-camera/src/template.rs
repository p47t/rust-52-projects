/// The HTML page template served at `/`. Includes rich dark-mode styles, glassmorphism,
/// live telemetry, and video controls (pause/resume, screenshot, fullscreen).
pub const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Rust IP Camera Server</title>
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;700&display=swap" rel="stylesheet">
    <style>
        :root {
            --bg-gradient: linear-gradient(135deg, #0b0f19 0%, #111827 50%, #1e1b4b 100%);
            --card-bg: rgba(17, 24, 39, 0.7);
            --card-border: rgba(255, 255, 255, 0.08);
            --text-main: #f9fafb;
            --text-muted: #9ca3af;
            --accent: #6366f1;
            --accent-hover: #4f46e5;
            --accent-glow: rgba(99, 102, 241, 0.4);
            --success: #10b981;
            --success-glow: rgba(16, 185, 129, 0.4);
            --font: 'Outfit', sans-serif;
        }
        
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        
        body {
            font-family: var(--font);
            background: var(--bg-gradient);
            color: var(--text-main);
            min-height: 100vh;
            display: flex;
            flex-direction: column;
            align-items: center;
            padding: 2.5rem 1.5rem;
            overflow-x: hidden;
        }

        .container {
            max-width: 1200px;
            width: 100%;
            display: flex;
            flex-direction: column;
            gap: 2rem;
        }

        header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            border-bottom: 1px solid var(--card-border);
            padding-bottom: 1.5rem;
        }

        h1 {
            font-weight: 700;
            font-size: 2.25rem;
            background: linear-gradient(to right, #a5b4fc, #818cf8, #6366f1);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            display: flex;
            align-items: center;
            gap: 0.75rem;
        }

        .live-indicator {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            background: rgba(16, 185, 129, 0.1);
            border: 1px solid var(--success);
            padding: 0.4rem 0.8rem;
            border-radius: 9999px;
            font-size: 0.75rem;
            font-weight: 700;
            color: var(--success);
            text-transform: uppercase;
            letter-spacing: 0.1em;
            box-shadow: 0 0 12px var(--success-glow);
        }

        .live-dot {
            width: 8px;
            height: 8px;
            background-color: var(--success);
            border-radius: 50%;
            animation: pulse 1.8s infinite;
        }

        @keyframes pulse {
            0% { transform: scale(0.95); opacity: 1; box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
            70% { transform: scale(1.1); opacity: 0.5; box-shadow: 0 0 0 8px rgba(16, 185, 129, 0); }
            100% { transform: scale(0.95); opacity: 1; box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
        }

        .dashboard {
            display: grid;
            grid-template-columns: 2.8fr 1.2fr;
            gap: 2rem;
        }

        @media (max-width: 900px) {
            .dashboard {
                grid-template-columns: 1fr;
            }
        }

        .feed-card {
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 1.5rem;
            backdrop-filter: blur(20px);
            padding: 1.5rem;
            display: flex;
            flex-direction: column;
            gap: 1.5rem;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
        }

        .video-wrapper {
            position: relative;
            width: 100%;
            aspect-ratio: 16/9;
            background: #030712;
            border-radius: 1rem;
            overflow: hidden;
            border: 1px solid rgba(255, 255, 255, 0.05);
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .video-feed {
            width: 100%;
            height: 100%;
            object-fit: contain;
            display: block;
        }

        .placeholder-text {
            color: var(--text-muted);
            text-align: center;
            display: none;
            flex-direction: column;
            align-items: center;
            gap: 1rem;
        }

        .placeholder-icon {
            font-size: 3.5rem;
            opacity: 0.7;
        }

        .controls {
            display: flex;
            gap: 1rem;
            flex-wrap: wrap;
        }

        .btn {
            font-family: var(--font);
            font-size: 0.9rem;
            font-weight: 600;
            padding: 0.75rem 1.5rem;
            border-radius: 0.75rem;
            border: 1px solid rgba(255, 255, 255, 0.1);
            background: rgba(255, 255, 255, 0.04);
            color: var(--text-main);
            cursor: pointer;
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }

        .btn:hover {
            background: rgba(255, 255, 255, 0.08);
            transform: translateY(-2px);
            border-color: rgba(255, 255, 255, 0.2);
        }

        .btn-primary {
            background: var(--accent);
            border-color: var(--accent);
            box-shadow: 0 4px 14px var(--accent-glow);
        }

        .btn-primary:hover {
            background: var(--accent-hover);
            border-color: var(--accent-hover);
            box-shadow: 0 6px 18px var(--accent-glow);
        }

        .sidebar {
            display: flex;
            flex-direction: column;
            gap: 2rem;
        }

        .telemetry-card {
            background: var(--card-bg);
            border: 1px solid var(--card-border);
            border-radius: 1.5rem;
            backdrop-filter: blur(20px);
            padding: 1.5rem;
            display: flex;
            flex-direction: column;
            gap: 1.5rem;
            box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
        }

        .card-title {
            font-size: 1.2rem;
            font-weight: 600;
            border-bottom: 1px solid var(--card-border);
            padding-bottom: 0.75rem;
            color: #a5b4fc;
            letter-spacing: 0.02em;
        }

        .stat-group {
            display: flex;
            flex-direction: column;
            gap: 1.25rem;
        }

        .stat-item {
            display: flex;
            flex-direction: column;
            gap: 0.25rem;
        }

        .stat-label {
            font-size: 0.75rem;
            text-transform: uppercase;
            letter-spacing: 0.08em;
            color: var(--text-muted);
        }

        .stat-value {
            font-size: 1.05rem;
            font-weight: 600;
        }

        .stat-value.highlight {
            color: #818cf8;
        }

        footer {
            margin-top: auto;
            text-align: center;
            padding: 2.5rem 0 1rem 0;
            font-size: 0.85rem;
            color: var(--text-muted);
            border-top: 1px solid var(--card-border);
        }

        footer a {
            color: var(--accent);
            text-decoration: none;
            font-weight: 600;
        }

        footer a:hover {
            text-decoration: underline;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="color: var(--accent);"><path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"></path><circle cx="12" cy="13" r="4"></circle></svg>
                Rust IP Camera Server
            </h1>
            <div class="live-indicator">
                <span class="live-dot"></span>
                <span>Live Feed</span>
            </div>
        </header>

        <div class="dashboard">
            <div class="feed-card">
                <div class="video-wrapper" id="videoWrapper">
                    <img src="/stream" class="video-feed" id="videoFeed" alt="MJPEG Stream">
                    <div class="placeholder-text" id="placeholderText">
                        <div class="placeholder-icon">🎥</div>
                        <p>Stream Paused</p>
                    </div>
                </div>
                <div class="controls">
                    <button class="btn btn-primary" id="toggleBtn" onclick="toggleStream()">
                        ⏸ Pause Stream
                    </button>
                    <button class="btn" onclick="takeSnapshot()">
                        📸 Capture Snapshot
                    </button>
                    <button class="btn" onclick="toggleFullscreen()">
                        📺 Fullscreen
                    </button>
                </div>
            </div>

            <div class="sidebar">
                <div class="telemetry-card">
                    <div class="card-title">System Telemetry</div>
                    <div class="stat-group">
                        <div class="stat-item">
                            <span class="stat-label">Host OS</span>
                            <span class="stat-value" id="platformVal">_OS_PLATFORM_</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">OpenCV Engine</span>
                            <span class="stat-value highlight">OpenCV 5.x Backend</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Web Engine</span>
                            <span class="stat-value">Axum & Tokio Async</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Stream Endpoint</span>
                            <span class="stat-value highlight">/stream</span>
                        </div>
                        <div class="stat-item">
                            <span class="stat-label">Server Uptime</span>
                            <span class="stat-value" id="uptimeVal">00:00:00</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <footer>
            <p>Powered by Rust & OpenCV 5. Pairs with V4L2 on Linux and MediaFoundation on Windows.</p>
        </footer>
    </div>

    <script>
        const videoFeed = document.getElementById('videoFeed');
        const toggleBtn = document.getElementById('toggleBtn');
        const placeholderText = document.getElementById('placeholderText');
        const videoWrapper = document.getElementById('videoWrapper');
        let isStreaming = true;

        function toggleStream() {
            if (isStreaming) {
                videoFeed.src = "";
                videoFeed.style.display = 'none';
                placeholderText.style.display = 'flex';
                toggleBtn.innerHTML = '▶ Resume Stream';
                toggleBtn.classList.remove('btn-primary');
                isStreaming = false;
            } else {
                videoFeed.src = "/stream?" + new Date().getTime();
                videoFeed.style.display = 'block';
                placeholderText.style.display = 'none';
                toggleBtn.innerHTML = '⏸ Pause Stream';
                toggleBtn.classList.add('btn-primary');
                isStreaming = true;
            }
        }

        function takeSnapshot() {
            if (!isStreaming) return;
            const canvas = document.createElement('canvas');
            canvas.width = videoFeed.naturalWidth || 640;
            canvas.height = videoFeed.naturalHeight || 480;
            const ctx = canvas.getContext('2d');
            ctx.drawImage(videoFeed, 0, 0, canvas.width, canvas.height);
            
            const link = document.createElement('a');
            link.download = `snapshot_${new Date().toISOString().slice(0,19).replace(/[:T]/g, '_')}.jpg`;
            link.href = canvas.toDataURL('image/jpeg');
            link.click();
        }

        function toggleFullscreen() {
            if (!document.fullscreenElement) {
                videoWrapper.requestFullscreen().catch(err => {
                    alert(`Error attempting to enable full-screen mode: ${err.message}`);
                });
            } else {
                document.exitFullscreen();
            }
        }

        const startTime = Date.now();
        setInterval(() => {
            const diff = Date.now() - startTime;
            const hrs = Math.floor(diff / 3600000).toString().padStart(2, '0');
            const mins = Math.floor((diff % 3600000) / 60000).toString().padStart(2, '0');
            const secs = Math.floor((diff % 60000) / 1000).toString().padStart(2, '0');
            document.getElementById('uptimeVal').innerText = `${hrs}:${mins}:${secs}`;
        }, 1000);
    </script>
</body>
</html>
"#;
