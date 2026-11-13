<script lang="ts">
    import { _ } from "svelte-i18n";
    import { onMount } from "svelte";
    import type { Track, Playlist } from "$lib/api/tauri";
    import {
        getPlaylistTracks,
        deletePlaylist,
        renamePlaylist,
        exportPlaylistZip,
        formatDuration,
    } from "$lib/api/tauri";
    import { confirm } from "$lib/stores/dialogs";
    import { contextMenu } from "$lib/stores/ui";
    import { buildPlaylistContextMenu } from "$lib/menus/contextMenus";
    import { playTracks, addToQueue, currentPlaylistId, isPlaying } from "$lib/stores/player";
    import { goToPlaylists, goToTracksMultiSelect } from "$lib/stores/view";
    import { loadPlaylists, playlists, playlistPendingTracks, drainPendingTracks } from "$lib/stores/library";
    import {
        playlistCovers,
        setPlaylistCover,
        removePlaylistCover,
    } from "$lib/stores/playlistCovers";
    import {
        downloadTracks,
        hasDownloadableTracks,
        needsDownloadLocation,
        showDownloadResult,
        type DownloadProgress,
    } from "$lib/services/downloadService";
    import { addToast } from "$lib/stores/toast";
    import Icon from "$lib/components/Icon.svelte";
    import DetailView from "./DetailView.svelte";

    export let playlistId: number;

    let playlist: Playlist | null = null;
    let tracks: Track[] = [];
    let loading = true;
    let menuOpen = false;

    // helper functions for SVG cover generation
    function initialsFromName(name: string) {
        if (!name) return "PL";
        const parts = name.trim().split(/\s+/);
        const picked = parts.slice(0, 2).map((p) => p[0]?.toUpperCase() ?? "");
        return picked.join("") || name.slice(0, 2).toUpperCase();
    }

    function hashToColor(str: string) {
        let h = 0;
        for (let i = 0; i < str.length; i++)
            h = (h << 5) - h + str.charCodeAt(i);
        const hue = Math.abs(h) % 360;
        return `hsl(${hue} 30% 30%)`;
    }

    function generateSvgCover(name: string, size = 512) {
        const initials = initialsFromName(name);
        const bg = hashToColor(name || "playlist");
        const svg =
            `<svg xmlns='http://www.w3.org/2000/svg' width='${size}' height='${size}' viewBox='0 0 ${size} ${size}'>` +
            `<rect width='100%' height='100%' fill='${bg}'/>` +
            `<text x='50%' y='50%' dominant-baseline='middle' text-anchor='middle' font-family='Inter, system-ui, sans-serif' font-size='${Math.floor(size / 3)}' fill='white' font-weight='700'>${initials}</text>` +
            `</svg>`;
        return `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(svg)))}`;
    }

    // Drain any dropped pending tracks
    $: if ($playlistPendingTracks[playlistId]?.length) {
        tracks = [...tracks, ...drainPendingTracks(playlistId)];
    }

    $: playlist = $playlists.find((p) => p.id === playlistId) || null;
    $: totalDuration = tracks.reduce((sum, t) => sum + (t.duration || 0), 0);

    // Reactive cover source
    $: coverSrc = (() => {
        if (!playlist) return generateSvgCover("Playlist");
        const custom = $playlistCovers && $playlistCovers[playlist.id];
        if (custom) return custom;
        if (playlist.cover_url) return playlist.cover_url;
        return generateSvgCover(playlist.name || "Playlist");
    })();

    $: hasCustomCover = !!(playlist && $playlistCovers && $playlistCovers[playlist.id]);

    function handleCoverUpload(file: File) {
        if (!file || !playlist) return;
        const reader = new FileReader();
        reader.onload = () => {
            const result = reader.result as string;
            if (result && playlist) {
                setPlaylistCover(playlist.id, result);
            }
        };
        reader.readAsDataURL(file);
    }

    async function handleRemoveCover() {
        if (!playlist) return;
        const ok = await confirm(
            $_("playlist.removeCoverConfirm", { values: { name: playlist.name } }),
            {
                title: $_("playlist.removeCoverTitle"),
                confirmLabel: $_("playlist.remove"),
                danger: true,
            },
        );
        if (!ok) return;
        removePlaylistCover(playlist.id);
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
            loadPlaylistData();
        } catch (error) {
            console.error("Download failed:", error);
            addToast($_("playlist.downloadFailed"), "error");
        } finally {
            isDownloading = false;
            downloadProgress = "";
        }
    }

    async function loadPlaylistData() {
        loading = true;
        try {
            tracks = await getPlaylistTracks(playlistId);
        } catch (error) {
            console.error("Failed to load playlist:", error);
        } finally {
            loading = false;
        }
    }

    function handlePlayAll() {
        if (tracks.length > 0) {
            playTracks(tracks, 0, {
                type: "playlist",
                playlistId: playlistId,
                displayName: playlist?.name ?? "Playlist",
            });
        }
    }

    function handleAddSongs() {
        goToTracksMultiSelect(playlistId);
    }

    async function handleDelete() {
        menuOpen = false;
        if (
            !(await confirm(
                $_("playlist.deleteConfirm", { values: { name: playlist?.name } }),
                {
                    title: $_("playlist.deletePlaylist"),
                    confirmLabel: $_("playlist.delete"),
                    danger: true,
                },
            ))
        )
            return;

        try {
            await deletePlaylist(playlistId);
            await loadPlaylists();
            goToPlaylists();
        } catch (error) {
            console.error("Failed to delete playlist:", error);
        }
    }

    async function handleRename(newName: string) {
        if (!newName.trim() || !playlist) return;
        try {
            await renamePlaylist(playlistId, newName.trim());
            await loadPlaylists();
        } catch (error) {
            console.error("Failed to rename playlist:", error);
        }
    }

    async function handleExportZip() {
        menuOpen = false;
        if (!playlist) return;
        try {
            addToast($_("playlist.exporting"), "info");
            const result = await exportPlaylistZip(playlistId, playlist.name);
            if (!result) return;
            const skipped_count = result.skipped_count;
            const exported = result.track_count - skipped_count;
            const skippedMsg = skipped_count > 0
                ? ` (${$_("playlist.exportedSkipped", { values: { count: skipped_count } })})`
                : "";
            addToast(
                `${$_("playlist.exportedTracks", { values: { count: exported } })}${skippedMsg}`,
                "success",
            );
        } catch (e) {
            addToast($_("playlist.exportFailed", { values: { error: String(e) } }), "error");
        }
    }

    function handleHeaderContextMenu(e: MouseEvent) {
        e.preventDefault();
        if (!playlist) return;
        contextMenu.set({
            visible: true,
            x: e.clientX,
            y: e.clientY,
            items: buildPlaylistContextMenu({
                playlist,
                tracks,
                variant: "detail",
                onPlay: handlePlayAll,
                onAddToQueue: () => {
                    if (tracks.length > 0) addToQueue(tracks);
                },
                onDelete: handleDelete,
                onExportZip: handleExportZip,
                t: $_,
            }),
        });
    }

    onMount(() => {
        loadPlaylistData();
    });

    $: playlistId, loadPlaylistData();
</script>

<DetailView
    {loading}
    notFound={!loading && !playlist}
    notFoundTitle={$_("playlist.notFound")}
    notFoundActionLabel={$_("playlist.backToPlaylists")}
    onNotFoundAction={goToPlaylists}
    title={playlist?.name ?? ""}
    type={$_("common.playlist")}
    typeIcon={playlist?.folder_path ? "folder" : undefined}
    coverSrc={coverSrc}
    coverAlt={$_("playlist.coverAlt")}
    coverShape="square"
    coverPlaceholderIcon="music"
    onBack={goToPlaylists}
    backAriaLabel="Back to Playlists"
    onHeaderContextMenu={handleHeaderContextMenu}
    editableTitle={true}
    onRename={handleRename}
    canEditCover={true}
    {hasCustomCover}
    onUploadCover={handleCoverUpload}
    onRemoveCover={handleRemoveCover}
    changeCoverTooltip={$_("playlist.changeCoverTooltip")}
    addCoverTooltip={$_("playlist.addCoverTooltip")}
    removeCoverTooltip={$_("playlist.removeCoverTooltip")}
    stats={[
        $_("playlist.songsCount", { values: { count: tracks.length } }),
        formatDuration(totalDuration),
    ]}
    onPlayAll={handlePlayAll}
    playAllDisabled={tracks.length === 0}
    isPlayingThis={$currentPlaylistId === playlistId && $isPlaying}
    {hasDownloadable}
    {allDownloaded}
    {isDownloading}
    {downloadProgress}
    onDownloadAll={handleDownloadAll}
    {tracks}
    {playlistId}
    showAlbum={true}
    playbackContext={{
        type: "playlist",
        playlistId,
        displayName: playlist?.name,
    }}
    onTracksUpdated={(updated) => {
        tracks = updated;
    }}
>
    <!-- Top-nav extra: options dropdown menu -->
    <svelte:fragment slot="header-extra">
        <div class="menu-container">
            <button
                class="icon-circle-btn"
                on:click={() => (menuOpen = !menuOpen)}
                title={$_("playlist.moreOptions")}
                aria-label={$_("playlist.moreOptions")}
                aria-haspopup="menu"
                aria-expanded={menuOpen}
            >
                <Icon name="more-horizontal" size={20} />
            </button>

            {#if menuOpen}
                <div class="menu-backdrop" role="presentation" on:click={() => (menuOpen = false)}></div>
                <div class="dropdown-menu" role="menu">
                    <button class="dropdown-item" role="menuitem" on:click={handleExportZip}>
                        <Icon name="folder" size={16} />
                        <span>{$_("contextMenu.exportToZip")}</span>
                    </button>
                    <div class="dropdown-divider"></div>
                    <button class="dropdown-item danger" role="menuitem" on:click={handleDelete}>
                        <Icon name="trash" size={16} />
                        <span>{$_("contextMenu.deletePlaylist")}</span>
                    </button>
                </div>
            {/if}
        </div>
    </svelte:fragment>

    <!-- Additional Action Button: Add Songs -->
    <svelte:fragment slot="actions">
        <button class="btn-action-outline add-songs-btn" on:click={handleAddSongs}>
            <Icon name="plus" size={18} />
            <span>{$_("playlist.addSongs")}</span>
        </button>
    </svelte:fragment>

    <!-- Empty State -->
    <svelte:fragment slot="empty">
        <div class="empty-playlist">
            <Icon name="music" size={54} />
            <h3>{$_("playlist.emptyTitle")}</h3>
            <p>{$_("playlist.emptyDescription")}</p>
            <button class="btn-primary add-songs-large" on:click={handleAddSongs}>
                <Icon name="plus" size={20} />
                <span>{$_("playlist.addSongs")}</span>
            </button>
        </div>
    </svelte:fragment>
</DetailView>

<style>
    .menu-container {
        position: relative;
    }

    .icon-circle-btn {
        width: 36px;
        height: 36px;
        border-radius: var(--radius-full);
        background: rgba(0, 0, 0, 0.4);
        color: var(--text-primary);
        border: none;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        transition: all var(--transition-fast);
    }

    .icon-circle-btn:hover {
        background: rgba(0, 0, 0, 0.7);
        transform: scale(1.08);
    }

    .menu-backdrop {
        position: fixed;
        inset: 0;
        z-index: 99;
    }

    .dropdown-menu {
        position: absolute;
        top: calc(100% + 8px);
        right: 0;
        z-index: 100;
        background: var(--bg-surface);
        border: 1px solid var(--border-color);
        border-radius: var(--radius-md);
        box-shadow: var(--shadow-lg);
        min-width: 180px;
        padding: 4px 0;
        display: flex;
        flex-direction: column;
        animation: fadeIn 0.12s ease-out;
    }

    .dropdown-item {
        display: flex;
        align-items: center;
        gap: var(--spacing-sm);
        padding: var(--spacing-sm) var(--spacing-md);
        background: transparent;
        border: none;
        color: var(--text-primary);
        font-size: var(--font-size-sm);
        cursor: pointer;
        text-align: left;
        width: 100%;
        transition: background-color var(--transition-fast);
    }

    .dropdown-item:hover {
        background-color: var(--bg-highlight);
    }

    .dropdown-item.danger {
        color: var(--error-color);
    }

    .dropdown-item.danger:hover {
        background-color: rgba(241, 94, 108, 0.15);
    }

    .dropdown-divider {
        height: 1px;
        background-color: var(--border-color);
        margin: 4px 0;
    }

    .btn-action-outline {
        display: inline-flex;
        align-items: center;
        gap: var(--spacing-sm);
        padding: var(--spacing-sm) var(--spacing-lg);
        background: transparent;
        color: var(--text-primary);
        border: 1px solid var(--border-color);
        border-radius: var(--radius-full);
        font-size: var(--font-size-base);
        font-weight: var(--font-weight-semibold);
        cursor: pointer;
        transition: all var(--transition-fast);
    }

    .btn-action-outline:hover {
        border-color: var(--text-primary);
        transform: scale(1.04);
    }

    .empty-playlist {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: var(--spacing-xl) 0;
        gap: var(--spacing-md);
        color: var(--text-secondary);
        text-align: center;
    }

    .add-songs-large {
        display: inline-flex;
        align-items: center;
        gap: var(--spacing-sm);
        padding: var(--spacing-sm) var(--spacing-xl);
        background: var(--accent-primary);
        color: #000;
        font-size: var(--font-size-base);
        font-weight: var(--font-weight-bold);
        border-radius: var(--radius-full);
        border: none;
        cursor: pointer;
        transition: transform var(--transition-fast);
        margin-top: var(--spacing-sm);
    }

    .add-songs-large:hover {
        transform: scale(1.05);
        background: var(--accent-hover);
    }
</style>
