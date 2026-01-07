// Simple JSON state persistence for player settings
import { get } from 'svelte/store';
import { volume, currentTrack, queue, queueIndex } from './player';
import { lyricsVisible } from './lyrics';
import type { Track } from '$lib/api/tauri';

const STORAGE_KEY = 'rlist_player_state';

export interface PersistedState {
    volume: number;           // Slider value (0-1), linear
    lyricsVisible: boolean;
    lastTrack: {
        id: number;
        path: string;
        title: string | null;
        artist: string | null;
        album: string | null;
    } | null;
}

// Default state
const defaultState: PersistedState = {
    volume: 0.7,
    lyricsVisible: false,
    lastTrack: null
};

// Load state from localStorage
export function loadPersistedState(): PersistedState {
    if (typeof window === 'undefined') return defaultState;
    
    try {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored) {
            const parsed = JSON.parse(stored);
            return { ...defaultState, ...parsed };
        }
    } catch (error) {
        console.error('[Persist] Failed to load state:', error);
    }
    
    return defaultState;
}

// Save current state to localStorage
export function savePersistedState(): void {
    if (typeof window === 'undefined') return;
    
    try {
        const track = get(currentTrack);
        const state: PersistedState = {
            volume: get(volume),
            lyricsVisible: get(lyricsVisible),
            lastTrack: track ? {
                id: track.id,
                path: track.path,
                title: track.title,
                artist: track.artist,
                album: track.album
            } : null
        };
        
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch (error) {
        console.error('[Persist] Failed to save state:', error);
    }
}

// Initialize stores from persisted state
export function initializeFromPersistedState(): void {
    const state = loadPersistedState();
    
    volume.set(state.volume);
    lyricsVisible.set(state.lyricsVisible);
    
    // Note: We don't auto-play the last track, just store it for reference
    // The user can implement "resume" functionality if desired
}

// Auto-save on changes (debounced)
let saveTimeout: ReturnType<typeof setTimeout> | null = null;

export function scheduleStateSave(): void {
    if (saveTimeout) {
        clearTimeout(saveTimeout);
    }
    saveTimeout = setTimeout(() => {
        savePersistedState();
        saveTimeout = null;
    }, 1000); // Debounce 1 second
}

// Subscribe to store changes for auto-save
export function setupAutoSave(): void {
    volume.subscribe(() => scheduleStateSave());
    lyricsVisible.subscribe(() => scheduleStateSave());
    currentTrack.subscribe(() => scheduleStateSave());
}
