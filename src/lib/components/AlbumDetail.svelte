<script lang="ts">
    import { onMount } from "svelte";
    import type { Album, Track } from "$lib/api/tauri";
    import {
        getAlbum,
        getTracksByAlbum,
        formatDuration,
        getReleaseMbInfo,
        type MbReleaseInfo,
    } from "$lib/api/tauri";
    import { playTracks, currentAlbumId, isPlaying } from "$lib/stores/player";
    import { goToAlbums, goToArtistDetail } from "$lib/stores/view";
    import ArtistLinks from "$lib/components/ArtistLinks.svelte";
    import { getAlbumCoverFromTracks } from "$lib/stores/library";
    import {
        downloadTracks,
        hasDownloadableTracks,
        needsDownloadLocation,
        showDownloadResult,
        type DownloadProgress,
    } from "$lib/services/downloadService";
    import { addToast } from "$lib/stores/toast";
    import { _ } from "svelte-i18n";
    import Icon from "$lib/components/Icon.svelte";
    import DetailView from "./DetailView.svelte";
    import { contextMenu } from "$lib/stores/ui";
    import { buildAlbumContextMenu } from "$lib/menus/contextMenus";
    import { pluginStore } from "$lib/stores/plugin-store";

    export let albumId: number;

    let album: Album | null = null;
    let tracks: Track[] = [];
    let loading = true;

    // MusicBrainz release info
    let mbRelease: MbReleaseInfo | null = null;
    let mbReleaseLoading = false;

    $: totalDuration = tracks.reduce((sum, t) => sum + (t.duration || 0), 0);

    async function loadAlbumData() {
        loading = true;
        mbRelease = null;
        try {
            const [albumData, trackData] = await Promise.all([
                getAlbum(albumId),
                getTracksByAlbum(albumId),
            ]);
            album = albumData;
            // Sort by disc number, then track number for proper album order
            tracks = trackData.sort((a, b) => {
                const discA = a.disc_number || 1;
                const discB = b.disc_number || 1;
                if (discA !== discB) return discA - discB;
                return (a.track_number || 0) - (b.track_number || 0);
            });
            // Background MB fetch — don't await so it doesn't block the UI
            if (album) fetchMbRelease(album.name, album.artist || "");
        } catch (error) {
            console.error("Failed to load album:", error);
        } finally {
            loading = false;
        }
    }

    async function fetchMbRelease(name: string, artist: string) {
        if (!name) return;
        mbReleaseLoading = true;
        try {
            mbRelease = await getReleaseMbInfo(name, artist);
        } catch (e) {
            console.warn("[AlbumDetail] MB release fetch failed:", e);
        } finally {
            mbReleaseLoading = false;
        }
    }

    // Download state
    let isDownloading = false;
    let downloadProgress = "";

    $: downloadableTracks = tracks.filter((t) => {
        return hasDownloadableTracks([t]) && !t.local_src;
    });

    $: hasDownloadable = downloadableTracks.length > 0;

    $: allDownloaded =
        tracks.length > 0 &&
        tracks.every((t) => {
            if (!t.source_type || t.source_type === "local") return true;
            return !!t.local_src;
        });

    $: isTidalAvailable = $pluginStore.installed.some(
        (p) => p.name === "Tidal Search" && p.enabled,
    );
    $: shouldShowAlbum = tracks.length > 0 || isTidalAvailable;

    function formatBytes(bytes: number): string {
        if (bytes === 0) return "0 B";
        const k = 1024;
        const sizes = ["B", "KB", "MB", "GB"];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
    }

    async function handleDownloadAll() {
        if (isDownloading) return;

        if (needsDownloadLocation()) {
            addToast($_("playlist.configureDownloadLocation"), "error");
            return;
        }

        isDownloading = true;
        downloadProgress = $_("playlist.downloadStarting");

        try {
            const result = await downloadTracks(
                tracks,
                (progress: DownloadProgress) => {
                    const current = progress.current;
                    const total = progress.total;

                    if (progress.bytesTotal) {
                        const currentMB = formatBytes(progress.bytesCurrent || 0);
                        const totalMB = formatBytes(progress.bytesTotal);
                        downloadProgress = `${current}/${total} (${currentMB}/${totalMB})`;
                    } else {
                        downloadProgress = `${current}/${total}`;
                    }
                },
            );

            showDownloadResult(result);
        } catch (error) {
            console.error("Download failed:", error);
            addToast($_("playlist.downloadFailed"), "error");
        } finally {
            isDownloading = false;
            downloadProgress = "";
        }
    }

    function handlePlayAll() {
        if (tracks.length > 0 && album) {
            playTracks(tracks, 0, {
                type: "album",
                albumId: album.id,
                displayName: album.name,
            });
        }
    }

    function handleContextMenu(e: MouseEvent) {
        if (!album) return;
        e.preventDefault();
        contextMenu.set({
            visible: true,
            x: e.clientX,
            y: e.clientY,
            items: buildAlbumContextMenu({
                album,
                showPlay: true,
                showGoToArtist: true,
                showDelete: true,
                onPlay: handlePlayAll,
                onAfterDelete: () => goToAlbums(),
                t: $_,
            }),
        });
    }

    onMount(() => {
        loadAlbumData();
    });

    $: albumId, loadAlbumData();
</script>

<DetailView
    {loading}
    notFound={!loading && (!album || !shouldShowAlbum)}
    notFoundTitle={$_("album.notFound")}
    notFoundActionLabel={$_("album.backToAlbums")}
    onNotFoundAction={goToAlbums}
    title={album?.name ?? ""}
    type={$_("album.type")}
    typeIcon="disc"
    coverSrc={album ? getAlbumCoverFromTracks(album.id) : null}
    coverAlt={album?.name ?? ""}
    coverShape="square"
    coverPlaceholderIcon="disc"
    onBack={goToAlbums}
    backAriaLabel="Back to Albums"
    onHeaderContextMenu={handleContextMenu}
    stats={[
        $_("album.songs", { values: { count: tracks.length } }),
        formatDuration(totalDuration),
    ]}
    onPlayAll={handlePlayAll}
    playAllDisabled={tracks.length === 0}
    isPlayingThis={$currentAlbumId === albumId && $isPlaying}
    {hasDownloadable}
    {allDownloaded}
    {isDownloading}
    {downloadProgress}
    onDownloadAll={handleDownloadAll}
    {tracks}
    showAlbum={false}
    groupDiscs={true}
    playbackContext={{
        type: "album",
        albumId,
        displayName: album?.name,
    }}
>
    <!-- Subtitle: Artist links -->
    <svelte:fragment slot="subtitle">
        {#if album}
            <ArtistLinks
                artist={album.artist || $_("common.unknownArtist")}
                artists={album.artists}
                chipClass="album-artist link"
                wrapClass="album-artist-links"
                chipTitle={$_("contextMenu.goToArtist")}
                on:select={(e) => goToArtistDetail(e.detail)}
            />
        {/if}
    </svelte:fragment>

    <!-- musicbrainz tags inline with the stats row, same bullet separation -->
    <svelte:fragment slot="stats-extra">
        {#if mbReleaseLoading}
            <span class="stat-dot">•</span>
            <span class="mb-loading">
                <span class="mb-info-spinner"></span>
                <span class="mb-info-hint">{$_("album.fetchingReleaseInfo")}</span>
            </span>
        {:else if mbRelease}
            {#if mbRelease.release_type}
                <span class="stat-dot">•</span>
                <span class="mb-chip type-chip">{mbRelease.release_type}</span>
            {/if}
            {#if mbRelease.year}
                <span class="stat-dot">•</span>
                <span class="mb-chip">{mbRelease.year}</span>
            {/if}
            {#if mbRelease.label}
                <span class="stat-dot">•</span>
                <span class="mb-chip">
                    <Icon name="tag" size={12} />
                    {mbRelease.label}
                </span>
            {/if}
            {#if mbRelease.country}
                <span class="stat-dot">•</span>
                <span class="mb-chip">{mbRelease.country}</span>
            {/if}
        {/if}
    </svelte:fragment>
</DetailView>

<style>
    :global(.album-artist.link) {
        font-weight: var(--font-weight-semibold);
        color: var(--text-primary);
        background: none;
        border: none;
        padding: 0;
        cursor: pointer;
    }

    :global(.album-artist.link:hover) {
        text-decoration: underline;
    }

    /* ── MusicBrainz info bar ── */
    .mb-loading {
        display: inline-flex;
        align-items: center;
        opacity: 0.6;
    }

    .mb-info-spinner {
        width: 12px;
        height: 12px;
        border: 2px solid var(--border-color);
        border-top-color: var(--accent-primary);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
        display: inline-block;
        margin-right: 6px;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    .mb-info-hint {
        font-size: var(--font-size-xs);
        color: var(--text-subdued);
    }

    .mb-chip {
        display: inline-flex;
        align-items: center;
        gap: 4px;
        padding: 2px 8px;
        border-radius: var(--radius-full);
        background: rgba(255, 255, 255, 0.08);
        color: var(--text-secondary);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
    }

    .mb-chip.type-chip {
        background: rgba(29, 185, 84, 0.15);
        color: var(--accent-primary);
    }
</style>
