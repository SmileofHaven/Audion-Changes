// Library store - manages music library state
import { writable, derived } from 'svelte/store';
import type { Track, Album, Artist, Playlist } from '$lib/api/tauri';
import { getLibrary, getPlaylists } from '$lib/api/tauri';

// Track store
export const tracks = writable<Track[]>([]);

// Album store
export const albums = writable<Album[]>([]);

// Artist store
export const artists = writable<Artist[]>([]);

// Playlist store
export const playlists = writable<Playlist[]>([]);

// Loading state
export const isLoading = writable(false);

// Error state
export const lastError = writable<string | null>(null);

// Derived store for track count
export const trackCount = derived(tracks, $tracks => $tracks.length);

// Derived store for album count
export const albumCount = derived(albums, $albums => $albums.length);

// Derived store for artist count
export const artistCount = derived(artists, $artists => $artists.length);

// Load library from backend
export async function loadLibrary(): Promise<void> {
    isLoading.set(true);
    lastError.set(null);

    try {
        const library = await getLibrary();
        tracks.set(library.tracks);
        albums.set(library.albums);
        artists.set(library.artists);
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        lastError.set(message);
        console.error('Failed to load library:', error);
    } finally {
        isLoading.set(false);
    }
}

// Load playlists from backend
export async function loadPlaylists(): Promise<void> {
    try {
        const playlistList = await getPlaylists();
        playlists.set(playlistList);
    } catch (error) {
        console.error('Failed to load playlists:', error);
    }
}

// Refresh all data
export async function refreshAll(): Promise<void> {
    await Promise.all([loadLibrary(), loadPlaylists()]);
}

// Clear library data (for cache clearing)
export async function clearLibrary(): Promise<void> {
    tracks.set([]);
    albums.set([]);
    artists.set([]);
    playlists.set([]);
    lastError.set(null);
}
