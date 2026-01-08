// Play Counter Plugin
// Tracks play counts for each song and displays them in the UI

(function () {
    'use strict';

    const PlayCounter = {
        name: 'Play Counter',
        playCounts: {},
        lastTrackId: null,
        playStartTime: null,
        MIN_PLAY_TIME: 30000, // 30 seconds minimum to count as a play
        uiElement: null,
        playerBarButton: null,
        isMinimized: false,
        isVisible: true,

        init(api) {
            console.log('[PlayCounter] Plugin initialized');
            console.log('[PlayCounter] API received:', api);
            this.api = api;

            try {
                this.loadCounts();
                console.log('[PlayCounter] Counts loaded');

                this.injectStyles();
                console.log('[PlayCounter] About to create UI');

                this.createUI();
                console.log('[PlayCounter] UI creation completed');
            } catch (err) {
                console.error('[PlayCounter] Init error:', err);
            }

            // Create player bar button (with retry for late DOM loading)
            this.createPlayerBarButton();
            setTimeout(() => this.createPlayerBarButton(), 500);
            setTimeout(() => this.createPlayerBarButton(), 1500);

            // Monitor track changes
            this.checkInterval = setInterval(() => this.checkTrack(), 1000);
            // Update UI periodically
            this.uiInterval = setInterval(() => this.updateUI(), 500);
        },

        injectStyles() {
            try {
                console.log('[PlayCounter] Creating style element...');

                // Check if styles already exist
                if (document.getElementById('play-counter-styles')) {
                    console.log('[PlayCounter] Styles already injected, skipping');
                    return;
                }

                // Inject CSS for the play counter display
                const style = document.createElement('style');
                style.id = 'play-counter-styles';
                style.textContent = `
                #play-counter-widget {
                    position: fixed;
                    bottom: 120px;
                    right: 20px;
                    background: #181818;
                    backdrop-filter: blur(10px);
                    border-radius: 12px;
                    padding: 16px 20px;
                    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
                    z-index: 9999;
                    min-width: 240px;
                    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                    border: 1px solid #404040;
                    animation: slideIn 0.3s ease-out;
                    transition: all 250ms ease;
                }

                #play-counter-widget:hover {
                    background: #282828;
                    transform: translateY(-2px);
                    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
                }

                @keyframes slideIn {
                    from {
                        transform: translateX(300px);
                        opacity: 0;
                    }
                    to {
                        transform: translateX(0);
                        opacity: 1;
                    }
                }

                #play-counter-widget h3 {
                    margin: 0 0 12px 0;
                    font-size: 12px;
                    font-weight: 700;
                    color: #b3b3b3;
                    text-transform: uppercase;
                    letter-spacing: 1px;
                    display: flex;
                    align-items: center;
                    gap: 8px;
                }

                #play-counter-widget h3::before {
                    content: '📊';
                    font-size: 14px;
                }

                .play-counter-current {
                    background: #121212;
                    border-radius: 8px;
                    padding: 12px;
                    margin-bottom: 12px;
                    border: 1px solid #282828;
                    transition: all 150ms ease;
                }

                .play-counter-current:hover {
                    border-color: #1DB954;
                }

                .play-counter-track-name {
                    font-size: 13px;
                    font-weight: 500;
                    color: #ffffff;
                    margin-bottom: 6px;
                    white-space: nowrap;
                    overflow: hidden;
                    text-overflow: ellipsis;
                }

                .play-counter-count {
                    font-size: 28px;
                    font-weight: 700;
                    color: #1DB954;
                    text-align: center;
                    text-shadow: 0 2px 8px rgba(29, 185, 84, 0.3);
                }

                .play-counter-label {
                    font-size: 11px;
                    color: #b3b3b3;
                    text-align: center;
                    margin-top: 4px;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                }

                .play-counter-stats {
                    display: flex;
                    gap: 8px;
                    font-size: 11px;
                    color: #b3b3b3;
                }

                .play-counter-stat {
                    flex: 1;
                    background: #121212;
                    border-radius: 8px;
                    padding: 8px;
                    text-align: center;
                    border: 1px solid #282828;
                    transition: all 150ms ease;
                }

                .play-counter-stat:hover {
                    background: #282828;
                    border-color: #404040;
                }

                .play-counter-stat-value {
                    display: block;
                    font-size: 16px;
                    font-weight: 700;
                    color: #1ed760;
                    margin-bottom: 2px;
                }

                .play-counter-no-track {
                    text-align: center;
                    color: #6a6a6a;
                    font-size: 12px;
                    padding: 8px;
                }

                #play-counter-widget.minimized {
                    min-width: auto;
                    padding: 12px 16px;
                }

                #play-counter-widget.minimized .play-counter-content {
                    display: none;
                }

                #play-counter-widget.minimized h3 {
                    margin: 0;
                }

                .play-counter-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 12px;
                }

                #play-counter-widget.minimized .play-counter-header {
                    margin-bottom: 0;
                }

                .play-counter-minimize-btn {
                    background: rgba(255, 255, 255, 0.1);
                    border: none;
                    border-radius: 4px;
                    width: 20px;
                    height: 20px;
                    cursor: pointer;
                    color: #b3b3b3;
                    font-size: 14px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition: all 150ms ease;
                }

                .play-counter-minimize-btn:hover {
                    background: rgba(255, 255, 255, 0.2);
                    color: #ffffff;
                }
            `;
                document.head.appendChild(style);
                console.log('[PlayCounter] Styles injected successfully');
            } catch (err) {
                console.error('[PlayCounter] Style injection error:', err);
            }
        },

        createUI() {
            try {
                console.log('[PlayCounter] Creating widget element...');

                // Check if widget already exists in DOM
                let widget = document.getElementById('play-counter-widget');
                if (widget) {
                    console.log('[PlayCounter] Widget already exists, reusing it');
                    this.uiElement = widget;
                    return;
                }

                // Create the UI widget
                widget = document.createElement('div');
                widget.id = 'play-counter-widget';
                widget.innerHTML = `
                <div class="play-counter-header">
                    <h3>Play Counter</h3>
                    <button class="play-counter-minimize-btn" id="pc-minimize-btn" title="Minimize">−</button>
                </div>
                <div class="play-counter-content">
                    <div class="play-counter-current">
                        <div class="play-counter-track-name" id="pc-track-name">No track playing</div>
                        <div class="play-counter-count" id="pc-count">0</div>
                        <div class="play-counter-label">plays</div>
                    </div>
                    <div class="play-counter-stats">
                        <div class="play-counter-stat">
                            <span class="play-counter-stat-value" id="pc-total">0</span>
                            <span>Total Tracks</span>
                        </div>
                        <div class="play-counter-stat">
                            <span class="play-counter-stat-value" id="pc-total-plays">0</span>
                            <span>Total Plays</span>
                        </div>
                    </div>
                </div>
            `;
                console.log('[PlayCounter] Appending widget to body...');
                document.body.appendChild(widget);
                this.uiElement = widget;

                // Add minimize button listener
                const minimizeBtn = document.getElementById('pc-minimize-btn');
                if (minimizeBtn) {
                    minimizeBtn.addEventListener('click', () => this.toggleMinimize());
                }

                console.log('[PlayCounter] Widget appended, element:', this.uiElement);
            } catch (err) {
                console.error('[PlayCounter] UI creation error:', err);
            }
        },

        createPlayerBarButton() {
            try {
                // Check if button already exists
                if (document.getElementById('pc-player-bar-btn')) {
                    console.log('[PlayCounter] Player bar button already exists');
                    return;
                }

                // Find the volume-controls section
                const volumeControls = document.querySelector('.volume-controls');
                if (!volumeControls) {
                    console.log('[PlayCounter] Volume controls not found, will retry');
                    return;
                }

                // Create toggle button
                const button = document.createElement('button');
                button.id = 'pc-player-bar-btn';
                button.className = 'icon-btn active';
                button.title = 'Toggle Play Counter';
                button.innerHTML = `
                    <svg viewBox="0 0 24 24" fill="currentColor" width="20" height="20">
                        <path d="M5 9.2h3V19H5zM10.6 5h2.8v14h-2.8zm5.6 8H19v6h-2.8z"/>
                    </svg>
                `;
                button.addEventListener('click', () => this.toggleVisibility());

                // Insert before the volume separator (after Queue and Lyrics buttons)
                const separator = volumeControls.querySelector('.volume-separator');
                if (separator) {
                    volumeControls.insertBefore(button, separator);
                } else {
                    volumeControls.insertBefore(button, volumeControls.firstChild);
                }
                this.playerBarButton = button;

                console.log('[PlayCounter] Player bar button created');
            } catch (err) {
                console.error('[PlayCounter] Player bar button creation error:', err);
            }
        },

        toggleMinimize() {
            this.isMinimized = !this.isMinimized;
            if (this.uiElement) {
                if (this.isMinimized) {
                    this.uiElement.classList.add('minimized');
                    const minimizeBtn = document.getElementById('pc-minimize-btn');
                    if (minimizeBtn) {
                        minimizeBtn.innerHTML = '+';
                        minimizeBtn.title = 'Expand';
                    }
                } else {
                    this.uiElement.classList.remove('minimized');
                    const minimizeBtn = document.getElementById('pc-minimize-btn');
                    if (minimizeBtn) {
                        minimizeBtn.innerHTML = '−';
                        minimizeBtn.title = 'Minimize';
                    }
                }
            }
        },

        toggleVisibility() {
            this.isVisible = !this.isVisible;
            if (this.uiElement) {
                this.uiElement.style.display = this.isVisible ? 'block' : 'none';
            }
            if (this.playerBarButton) {
                if (this.isVisible) {
                    this.playerBarButton.classList.add('active');
                } else {
                    this.playerBarButton.classList.remove('active');
                }
            }
        },

        updateUI() {
            if (!this.uiElement || !this.api?.player?.getCurrentTrack) return;

            try {
                const track = this.api.player.getCurrentTrack();
                const trackNameEl = document.getElementById('pc-track-name');
                const countEl = document.getElementById('pc-count');
                const totalEl = document.getElementById('pc-total');
                const totalPlaysEl = document.getElementById('pc-total-plays');

                if (track) {
                    const count = this.getCount(track.id);
                    const trackName = track.title || 'Unknown Track';
                    const artist = track.artist || 'Unknown Artist';

                    trackNameEl.textContent = `${trackName} - ${artist}`;
                    countEl.textContent = count;
                } else {
                    trackNameEl.textContent = 'No track playing';
                    countEl.textContent = '0';
                }

                // Update stats
                const totalTracks = Object.keys(this.playCounts).length;
                const totalPlays = Object.values(this.playCounts).reduce((sum, count) => sum + count, 0);

                totalEl.textContent = totalTracks;
                totalPlaysEl.textContent = totalPlays;
            } catch (err) {
                console.error('[PlayCounter] UI update error:', err);
            }
        },

        async loadCounts() {
            if (!this.api?.storage?.get) return;

            try {
                const saved = await this.api.storage.get('playCounts');
                if (saved) {
                    this.playCounts = JSON.parse(saved);
                    console.log('[PlayCounter] Loaded counts:', Object.keys(this.playCounts).length);
                }
            } catch (err) {
                console.error('[PlayCounter] Failed to load counts:', err);
            }
        },

        async saveCounts() {
            if (!this.api?.storage?.set) return;

            try {
                await this.api.storage.set('playCounts', JSON.stringify(this.playCounts));
            } catch (err) {
                console.error('[PlayCounter] Failed to save counts:', err);
            }
        },

        async checkTrack() {
            if (!this.api?.player?.getCurrentTrack) return;

            try {
                const track = this.api.player.getCurrentTrack();
                const isPlaying = this.api.player.isPlaying?.();

                if (!track) return;

                // New track started
                if (track.id !== this.lastTrackId) {
                    // Count previous track if played long enough
                    if (this.lastTrackId && this.playStartTime) {
                        const playDuration = Date.now() - this.playStartTime;
                        if (playDuration >= this.MIN_PLAY_TIME) {
                            this.incrementCount(this.lastTrackId);
                        }
                    }

                    this.lastTrackId = track.id;
                    this.playStartTime = isPlaying ? Date.now() : null;
                } else if (isPlaying && !this.playStartTime) {
                    // Track resumed
                    this.playStartTime = Date.now();
                } else if (!isPlaying && this.playStartTime) {
                    // Track paused
                    this.playStartTime = null;
                }
            } catch (err) {
                console.error('[PlayCounter] Error:', err);
            }
        },

        incrementCount(trackId) {
            this.playCounts[trackId] = (this.playCounts[trackId] || 0) + 1;
            console.log(`[PlayCounter] Track ${trackId} played ${this.playCounts[trackId]} times`);
            this.saveCounts();
            this.updateUI(); // Update UI immediately when count changes
        },

        getCount(trackId) {
            return this.playCounts[trackId] || 0;
        },

        getTopTracks(limit = 10) {
            return Object.entries(this.playCounts)
                .sort((a, b) => b[1] - a[1])
                .slice(0, limit);
        },

        start() {
            console.log('[PlayCounter] Plugin started');
            if (this.uiElement) {
                this.uiElement.style.display = 'block';
            }
        },

        stop() {
            console.log('[PlayCounter] Plugin stopped');
            if (this.uiElement) {
                this.uiElement.style.display = 'none';
            }
        },

        destroy() {
            if (this.checkInterval) {
                clearInterval(this.checkInterval);
            }
            if (this.uiInterval) {
                clearInterval(this.uiInterval);
            }
            if (this.uiElement) {
                this.uiElement.remove();
            }
            if (this.playerBarButton) {
                this.playerBarButton.remove();
            }
            const styleEl = document.getElementById('play-counter-styles');
            if (styleEl) {
                styleEl.remove();
            }
            this.saveCounts();
            console.log('[PlayCounter] Plugin destroyed');
        }
    };

    // Register plugin globally
    window.PlayCounter = PlayCounter;
    window.AudionPlugin = PlayCounter;
})();
