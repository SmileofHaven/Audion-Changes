<script lang="ts">
    import { onMount, onDestroy, tick } from "svelte";
    import { _ } from "svelte-i18n";
    import Icon from "$lib/components/Icon.svelte";
    import ArtistLinks from "$lib/components/ArtistLinks.svelte";
    import MetadataModal from "$lib/components/MetadataModal.svelte";
    import { paletteForCover } from "$lib/stores/palette";
    import { pickVibrantColor } from "$lib/stores/theme";
    import type { Track } from "$lib/api/tauri";
    import { formatDuration } from "$lib/api/tauri";
    import {
        playTracks,
        currentTrack,
        isPlaying,
        togglePlay,
        type PlaybackContext,
    } from "$lib/stores/player";
    import { likedTrackIds, toggleLike } from "$lib/stores/liked";
    import { contextMenu } from "$lib/stores/ui";
    import { buildTrackContextMenu, isTrackUnavailable } from "$lib/menus/contextMenus";
    import { goToAlbumDetail, goToArtistDetail } from "$lib/stores/view";
    import { isMobile } from "$lib/stores/mobile";

    // ──────────────────────────────────────────────
    // Props
    // ──────────────────────────────────────────────

    // General status
    export let loading = false;
    export let notFound = false;
    export let notFoundTitle = "";
    export let notFoundActionLabel = "";
    export let onNotFoundAction: (() => void) | undefined = undefined;

    // Header / Hero details
    export let title = "";
    export let type = "";
    export let typeIcon: string | undefined = undefined;
    export let coverSrc: string | null = null;
    export let coverAlt = "";
    export let coverShape: "square" | "circle" = "square";
    export let avatarInitials = "";
    export let coverPlaceholderIcon = "disc";
    export let onBack: (() => void) | undefined = undefined;
    export let backAriaLabel = "Go back";
    export let onHeaderContextMenu: ((e: MouseEvent) => void) | undefined = undefined;

    // Editable title (e.g. Playlist rename)
    export let editableTitle = false;
    export let isEditingTitle = false;
    export let onRename: ((newName: string) => void | Promise<void>) | undefined = undefined;

    // Editable cover (e.g. Playlist cover upload/delete)
    export let canEditCover = false;
    export let hasCustomCover = false;
    export let onUploadCover: ((file: File) => void) | undefined = undefined;
    export let onRemoveCover: (() => void) | undefined = undefined;
    export let changeCoverTooltip = "Change cover";
    export let addCoverTooltip = "Add cover";
    export let removeCoverTooltip = "Remove cover";

    // Metadata stats
    export let stats: string[] = [];

    // Action buttons
    export let onPlayAll: (() => void) | undefined = undefined;
    export let playAllDisabled = false;
    export let isPlayingThis = false;
    export let hasDownloadable = false;
    export let allDownloaded = false;
    export let isDownloading = false;
    export let downloadProgress = "";
    export let onDownloadAll: (() => void) | undefined = undefined;

    // Tabs
    export let tabs: { id: string; label: string; count?: number }[] = [];
    export let activeTab = "";
    export let onTabChange: ((tabId: string) => void) | undefined = undefined;

    // Tracks & Virtualized Grid
    export let tracks: Track[] = [];
    export let queueTracks: Track[] | null = null;
    export let playbackContext: PlaybackContext | undefined = undefined;
    export let showAlbum = true;
    export let showArtist = true;
    export let groupDiscs = false;
    export let playlistId: number | null = null;
    export let onTracksUpdated: ((updatedTracks: Track[]) => void) | undefined = undefined;

    // ──────────────────────────────────────────────
    // Internal State
    // ──────────────────────────────────────────────
    let containerEl: HTMLDivElement;
    let tracklistHeaderEl: HTMLDivElement;
    let tracksSpacerEl: HTMLDivElement;
    let fileInputEl: HTMLInputElement;
    let editTitleInput: HTMLInputElement;
    let editTitleValue = "";
    let coverHovered = false;
    let scrollTop = 0;
    let containerHeight = 600;
    let tracksTopOffset = 360;
    // header is transparent so the backdrop bleeds through, opaque once it sticks under the top bar
    $: headerStuck = scrollTop + 56 >= tracksTopOffset - 38;
    let resizeObserver: ResizeObserver | undefined;

    // backdrop color from the same rust palette extraction the player uses
    let backdropColor: string | null = null;
    let backdropGen = 0;

    async function updateBackdrop(src: string | null) {
        const gen = ++backdropGen;
        if (!src) {
            backdropColor = null;
            return;
        }
        const palette = await paletteForCover(src);
        if (gen !== backdropGen) return;
        backdropColor = pickVibrantColor(palette);
    }

    $: updateBackdrop(coverSrc);

    // Metadata modal state for track context menu
    let metadataModalTrack: Track | null = null;

    // Track playback reactive state
    $: playingTrackId = $currentTrack?.id ?? null;
    $: playing = $isPlaying;

    // ──────────────────────────────────────────────
    // Sorting State & Logic
    // ──────────────────────────────────────────────
    type SortField = "track_number" | "title" | "album" | "duration" | null;
    let sortField: SortField = null;
    let sortDirection: "asc" | "desc" = "asc";

    function toggleSort(field: SortField) {
        if (sortField === field) {
            if (sortDirection === "asc") {
                sortDirection = "desc";
            } else {
                sortField = null;
                sortDirection = "asc";
            }
        } else {
            sortField = field;
            sortDirection = "asc";
        }
    }

    $: sortedTracks = (() => {
        if (!sortField) return tracks;
        return [...tracks].sort((a, b) => {
            let valA: any = "";
            let valB: any = "";

            switch (sortField) {
                case "track_number":
                    valA = a.track_number ?? a.id;
                    valB = b.track_number ?? b.id;
                    break;
                case "title":
                    valA = (a.title || "").toLowerCase();
                    valB = (b.title || "").toLowerCase();
                    break;
                case "album":
                    valA = (a.album || "").toLowerCase();
                    valB = (b.album || "").toLowerCase();
                    break;
                case "duration":
                    valA = a.duration || 0;
                    valB = b.duration || 0;
                    break;
            }

            if (valA < valB) return sortDirection === "asc" ? -1 : 1;
            if (valA > valB) return sortDirection === "asc" ? 1 : -1;
            return 0;
        });
    })();

    // ──────────────────────────────────────────────
    // Virtualized Track Grid Setup
    // ──────────────────────────────────────────────
    type VirtualItem =
        | { type: "disc-header"; id: string; disc: number; height: number }
        | { type: "track"; id: number; track: Track; index: number; trackNumber: number; height: number };

    $: isMobileView = $isMobile;
    $: trackRowHeight = isMobileView ? 60 : 52;
    const DISC_HEADER_HEIGHT = 56;
    const OVERSCAN_PX = 320;

    $: hasMultipleDiscs = groupDiscs && tracks.some((t) => (t.disc_number || 1) > 1);

    // Build virtual item list
    let virtualItems: VirtualItem[] = [];
    $: {
        if (hasMultipleDiscs && !sortField) {
            const items: VirtualItem[] = [];
            let currentDisc = -1;
            let discTrackCounter = 0;

            for (let i = 0; i < tracks.length; i++) {
                const track = tracks[i];
                const disc = track.disc_number || 1;
                if (disc !== currentDisc) {
                    currentDisc = disc;
                    discTrackCounter = 0;
                    items.push({
                        type: "disc-header",
                        id: `disc-header-${disc}`,
                        disc,
                        height: DISC_HEADER_HEIGHT,
                    });
                }
                discTrackCounter++;
                items.push({
                    type: "track",
                    id: track.id,
                    track,
                    index: i,
                    trackNumber: track.track_number || discTrackCounter,
                    height: trackRowHeight,
                });
            }
            virtualItems = items;
        } else {
            virtualItems = sortedTracks.map((track, i) => ({
                type: "track",
                id: track.id,
                track,
                index: i,
                trackNumber: track.track_number || (i + 1),
                height: trackRowHeight,
            }));
        }
    }

    // Precompute offsets for virtual items
    let itemOffsets: { top: number; height: number; item: VirtualItem }[] = [];
    let totalTracksHeight = 0;

    $: {
        let currentTop = 0;
        itemOffsets = virtualItems.map((item) => {
            const top = currentTop;
            currentTop += item.height;
            return { top, height: item.height, item };
        });
        totalTracksHeight = currentTop;
    }

    // Calculate visible slice of items based on container scroll
    let visibleItems: { top: number; height: number; item: VirtualItem }[] = [];

    $: {
        // Effective scroll within tracklist
        const viewportTop = Math.max(0, scrollTop - tracksTopOffset);
        const viewportBottom = viewportTop + containerHeight;

        const minTop = viewportTop - OVERSCAN_PX;
        const maxTop = viewportBottom + OVERSCAN_PX;

        visibleItems = itemOffsets.filter(
            (entry) => entry.top + entry.height >= minTop && entry.top <= maxTop
        );
    }

    // ──────────────────────────────────────────────
    // Event Handlers
    // ──────────────────────────────────────────────

    function handleScroll(e: Event) {
        scrollTop = (e.target as HTMLElement).scrollTop;
    }

    function measureLayout() {
        if (!containerEl) return;
        containerHeight = containerEl.clientHeight;
        // spacer is not sticky => its rect gives the true row 0 offset in the scroll area
        if (tracksSpacerEl) {
            tracksTopOffset =
                tracksSpacerEl.getBoundingClientRect().top -
                containerEl.getBoundingClientRect().top +
                containerEl.scrollTop;
        }
    }

    onMount(() => {
        measureLayout();
        if (typeof ResizeObserver !== "undefined" && containerEl) {
            resizeObserver = new ResizeObserver(measureLayout);
            resizeObserver.observe(containerEl);
        }
    });

    onDestroy(() => {
        resizeObserver?.disconnect();
        resizeObserver = undefined;
    });

    // Re-measure layout when tracks, tabs, or activeTab changes
    $: if (tracks || activeTab) {
        tick().then(measureLayout);
    }

    function handlePlayTrack(track: Track, index: number) {
        const activeQueue = sortField ? sortedTracks : (queueTracks && queueTracks.length > 0 ? queueTracks : tracks);
        const queueIdx = activeQueue.findIndex((t) => t.id === track.id);
        playTracks(activeQueue, queueIdx !== -1 ? queueIdx : index, playbackContext);
    }

    function handleTrackToggle(track: Track, index: number) {
        if (playingTrackId === track.id) {
            togglePlay();
        } else {
            handlePlayTrack(track, index);
        }
    }

    function handleRowDoubleClick(track: Track, index: number) {
        handlePlayTrack(track, index);
    }

    function handleTrackContextMenu(track: Track, index: number, e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        const activeQueue = sortField ? sortedTracks : (queueTracks || tracks);
        const queueIdx = activeQueue.findIndex((t) => t.id === track.id);
        contextMenu.set({
            visible: true,
            x: e.clientX,
            y: e.clientY,
            items: buildTrackContextMenu({
                track,
                trackIndex: queueIdx !== -1 ? queueIdx : index,
                sortedTracks: activeQueue,
                queueTracks: activeQueue,
                playbackContext,
                playlistId,
                isUnavailable: isTrackUnavailable(track),
                onMetadataOpen: (t) => {
                    metadataModalTrack = t;
                },
                onTracksUpdated: (updated) => {
                    if (onTracksUpdated) onTracksUpdated(updated);
                },
                t: $_,
            }),
        });
    }

    function handleToggleLike(track: Track, e: MouseEvent) {
        e.stopPropagation();
        toggleLike(track.id);
    }

    function handleArtistClick(name: string, e?: Event) {
        if (e && typeof (e as any).stopPropagation === "function") {
            (e as any).stopPropagation();
        }
        if (name) goToArtistDetail(name);
    }

    function handleAlbumClick(albumId: number | null | undefined, e?: MouseEvent) {
        if (e) e.stopPropagation();
        if (albumId) goToAlbumDetail(albumId);
    }

    // Inline title editing
    function startTitleEdit() {
        if (!editableTitle) return;
        editTitleValue = title;
        isEditingTitle = true;
        tick().then(() => editTitleInput?.focus());
    }

    async function commitTitleEdit() {
        if (!isEditingTitle) return;
        const trimmed = editTitleValue.trim();
        if (trimmed && trimmed !== title && onRename) {
            await onRename(trimmed);
        }
        isEditingTitle = false;
    }

    function handleTitleKeydown(e: KeyboardEvent) {
        if (e.key === "Enter") {
            commitTitleEdit();
        } else if (e.key === "Escape") {
            isEditingTitle = false;
        }
    }

    // Cover upload & removal
    function handleCoverFileChange(e: Event) {
        const input = e.target as HTMLInputElement;
        const file = input?.files?.[0];
        if (file && onUploadCover) {
            onUploadCover(file);
        }
        if (input) input.value = "";
    }

    function handleTabClick(tabId: string) {
        activeTab = tabId;
        if (onTabChange) onTabChange(tabId);
    }
</script>

<div
    class="detail-view"
    bind:this={containerEl}
    on:scroll={handleScroll}
>
    {#if loading}
        <div class="state-container">
            <div class="loading-spinner"></div>
            <span class="state-text">{$_("common.loading") || "Loading..."}</span>
        </div>
    {:else if notFound}
        <div class="state-container">
            <Icon name="alert-circle" size={48} />
            <h2>{notFoundTitle || $_("common.notFound") || "Not Found"}</h2>
            {#if onNotFoundAction}
                <button class="btn-action" on:click={onNotFoundAction}>
                    {notFoundActionLabel || $_("common.back") || "Go Back"}
                </button>
            {/if}
        </div>
    {:else}
        <!-- cover color tint, fades out downward and bleeds into the tracklist -->
        {#if backdropColor}
            <div class="hero-backdrop" style="--backdrop-color: {backdropColor}" aria-hidden="true"></div>
        {/if}

        <!-- Top Navigation Bar (Back & Extra actions) -->
        <div class="top-nav-bar" class:scrolled={scrollTop > 180}>
            {#if onBack}
                <button
                    class="nav-back-btn"
                    on:click={onBack}
                    aria-label={backAriaLabel}
                    title={backAriaLabel}
                >
                    <Icon name="arrow-left" size={20} />
                </button>
            {/if}

            <div class="nav-scrolled-info" class:visible={scrollTop > 220}>
                {#if coverSrc}
                    <img
                        src={coverSrc}
                        alt=""
                        class="nav-scrolled-thumb"
                        class:circle={coverShape === "circle"}
                    />
                {/if}
                <span class="nav-scrolled-title truncate">{title}</span>
                {#if onPlayAll}
                    <button
                        class="nav-play-pill"
                        on:click={onPlayAll}
                        disabled={playAllDisabled}
                        aria-label={$_("common.play") || "Play"}
                    >
                        <Icon name={isPlayingThis ? "pause" : "play"} size={16} />
                    </button>
                {/if}
            </div>

            <div class="top-nav-extra">
                <slot name="header-extra" />
            </div>
        </div>

        <!-- Hero Header -->
        <header
            class="hero-header"
            on:contextmenu={onHeaderContextMenu}
        >
            <!-- Cover / Avatar -->
            <div
                class="hero-cover-container"
                class:circle={coverShape === "circle"}
                on:mouseenter={() => (coverHovered = true)}
                on:mouseleave={() => (coverHovered = false)}
                role="presentation"
            >
                {#if coverSrc}
                    <img
                        src={coverSrc}
                        alt={coverAlt || title}
                        class="hero-cover-img"
                        class:circle={coverShape === "circle"}
                        decoding="async"
                    />
                {:else if avatarInitials}
                    <div class="hero-cover-placeholder circle">
                        <span class="avatar-initials">{avatarInitials}</span>
                    </div>
                {:else}
                    <div class="hero-cover-placeholder" class:circle={coverShape === "circle"}>
                        <Icon name={coverPlaceholderIcon} size={64} />
                    </div>
                {/if}

                {#if canEditCover}
                    <input
                        type="file"
                        accept="image/*"
                        bind:this={fileInputEl}
                        on:change={handleCoverFileChange}
                        style="display: none;"
                    />
                    {#if coverHovered}
                        <div class="cover-hover-overlay">
                            {#if hasCustomCover && onRemoveCover}
                                <button
                                    class="cover-action-btn danger"
                                    on:click={onRemoveCover}
                                    title={removeCoverTooltip}
                                    aria-label={removeCoverTooltip}
                                >
                                    <Icon name="trash" size={18} />
                                </button>
                            {/if}
                            <button
                                class="cover-action-btn"
                                on:click={() => fileInputEl?.click()}
                                title={hasCustomCover ? changeCoverTooltip : addCoverTooltip}
                                aria-label={hasCustomCover ? changeCoverTooltip : addCoverTooltip}
                            >
                                <Icon name={hasCustomCover ? "edit" : "plus"} size={18} />
                            </button>
                        </div>
                    {/if}
                {/if}
            </div>

            <!-- Hero Meta Details -->
            <div class="hero-info">
                {#if type}
                    <div class="hero-type-eyebrow">
                        {#if typeIcon}
                            <Icon name={typeIcon} size={14} />
                        {/if}
                        <span>{type}</span>
                    </div>
                {/if}

                {#if editableTitle && isEditingTitle}
                    <input
                        type="text"
                        class="hero-title-input"
                        bind:this={editTitleInput}
                        bind:value={editTitleValue}
                        on:keydown={handleTitleKeydown}
                        on:blur={commitTitleEdit}
                    />
                {:else}
                    <h1
                        class="hero-title"
                        class:editable={editableTitle}
                        on:dblclick={startTitleEdit}
                        title={editableTitle ? "Double click to rename" : undefined}
                    >
                        {title}
                    </h1>
                {/if}

                <!-- Subtitle / Artist slot -->
                <div class="hero-subtitle">
                    <slot name="subtitle" />
                </div>

                <!-- Stats line (song count, duration, albums, etc.) -->
                {#if stats.length > 0}
                    <div class="hero-stats">
                        {#each stats as stat, i}
                            {#if i > 0}
                                <span class="stat-dot">•</span>
                            {/if}
                            <span class="stat-item">{stat}</span>
                        {/each}
                        <slot name="stats-extra" />
                    </div>
                {/if}

                <!-- Extra Info Bar (e.g. MusicBrainz release tags) -->
                <slot name="info-bar" />
            </div>
        </header>

        <!-- Main Actions Bar -->
        <div class="action-bar">
            {#if onPlayAll}
                <button
                    class="btn-play-all"
                    on:click={onPlayAll}
                    disabled={playAllDisabled}
                    aria-label={$_("common.play") || "Play"}
                >
                    <Icon name={isPlayingThis ? "pause" : "play"} size={22} />
                    <span>{$_("common.play") || "Play"}</span>
                </button>
            {/if}

            {#if hasDownloadable || allDownloaded}
                <button
                    class="btn-action-outline"
                    class:downloaded={allDownloaded}
                    on:click={onDownloadAll}
                    disabled={isDownloading || (!hasDownloadable && !allDownloaded) || allDownloaded}
                >
                    {#if isDownloading}
                        <div class="btn-spinner"></div>
                        <span>{downloadProgress || $_("playlist.downloadStarting") || "Downloading..."}</span>
                    {:else if allDownloaded}
                        <Icon name="check-circle" size={18} />
                        <span>{$_("album.downloaded") || "Downloaded"}</span>
                    {:else}
                        <Icon name="download" size={18} />
                        <span>{$_("album.download") || "Download"}</span>
                    {/if}
                </button>
            {/if}

            <slot name="actions" />
        </div>

        <!-- Optional Tabs (e.g. for Artist Detail) -->
        {#if tabs.length > 0}
            <div class="tabs-container" role="tablist">
                {#each tabs as tab}
                    <button
                        class="tab-btn"
                        class:active={activeTab === tab.id}
                        role="tab"
                        aria-selected={activeTab === tab.id}
                        on:click={() => handleTabClick(tab.id)}
                    >
                        {tab.label}
                        {#if tab.count !== undefined}
                            <span class="tab-count">{tab.count}</span>
                        {/if}
                    </button>
                {/each}
            </div>
        {/if}

        <!-- Content Area: Custom Tab Content OR Built-in Virtualized Track Grid -->
        {#if tabs.length > 0 && activeTab && activeTab !== "tracks"}
            <div class="custom-tab-content">
                <slot name="tab-content" />
            </div>
        {:else}
            <!-- Track List Section with Virtualized Grid -->
            <div class="tracklist-container">
                {#if tracks.length > 0}
                    <!-- Column Header (Sticky) -->
                    <div
                        class="tracklist-header"
                        class:no-album={!showAlbum}
                        class:stuck={headerStuck}
                        bind:this={tracklistHeaderEl}
                        role="row"
                    >
                        <button
                            type="button"
                            class="col-header-btn col-num"
                            class:active-sort={sortField === "track_number"}
                            on:click={() => toggleSort("track_number")}
                            title={$_("trackList.sortByTrackNumber") || "Sort by track number"}
                            aria-label="Sort by track number"
                        >
                            <span>#</span>
                            {#if sortField === "track_number"}
                                <span class="sort-arrow" aria-hidden="true">{sortDirection === "asc" ? "▲" : "▼"}</span>
                            {/if}
                        </button>

                        <button
                            type="button"
                            class="col-header-btn col-title"
                            class:active-sort={sortField === "title"}
                            on:click={() => toggleSort("title")}
                            title={$_("trackList.sortByTitle") || "Sort by title"}
                            aria-label="Sort by title"
                        >
                            <span>{$_("trackList.title") || "Title"}</span>
                            {#if sortField === "title"}
                                <span class="sort-arrow" aria-hidden="true">{sortDirection === "asc" ? "▲" : "▼"}</span>
                            {/if}
                        </button>

                        {#if showAlbum}
                            <button
                                type="button"
                                class="col-header-btn col-album"
                                class:active-sort={sortField === "album"}
                                on:click={() => toggleSort("album")}
                                title={$_("trackList.sortByAlbum") || "Sort by album"}
                                aria-label="Sort by album"
                            >
                                <span>{$_("trackList.album") || "Album"}</span>
                                {#if sortField === "album"}
                                    <span class="sort-arrow" aria-hidden="true">{sortDirection === "asc" ? "▲" : "▼"}</span>
                                {/if}
                            </button>
                        {/if}

                        <button
                            type="button"
                            class="col-header-btn col-duration"
                            class:active-sort={sortField === "duration"}
                            on:click={() => toggleSort("duration")}
                            title={$_("trackList.sortByDuration") || "Sort by duration"}
                            aria-label="Sort by duration"
                        >
                            <Icon name="timer" size={15} />
                            {#if sortField === "duration"}
                                <span class="sort-arrow" aria-hidden="true">{sortDirection === "asc" ? "▲" : "▼"}</span>
                            {/if}
                        </button>

                        <span class="col-actions"></span>
                    </div>

                    <!-- Virtualized Track Rows Spacer -->
                    <div
                        class="tracks-virtual-spacer"
                        bind:this={tracksSpacerEl}
                        style="height: {totalTracksHeight}px;"
                    >
                        {#each visibleItems as { top, height, item } (item.type === "disc-header" ? item.id : item.track.id)}
                            {#if item.type === "disc-header"}
                                <!-- disc separator reuses the track row grid: icon in # column, label in title column -->
                                <div
                                    class="track-row disc-row"
                                    class:no-album={!showAlbum}
                                    style="transform: translateY({top}px); height: {height}px;"
                                >
                                    <span class="col-num">
                                        <Icon name="disc" size={26} />
                                    </span>
                                    <div class="col-title">
                                        <div class="title-line truncate">
                                            <span class="disc-title-text">
                                                {$_("album.disc", { values: { number: item.disc } }) || `Disc ${item.disc}`}
                                            </span>
                                        </div>
                                    </div>
                                </div>
                            {:else}
                                {@const track = item.track}
                                {@const isTrackPlaying = playingTrackId === track.id && playing}
                                {@const isTrackPaused = playingTrackId === track.id && !playing}
                                {@const isTrackLiked = $likedTrackIds.has(track.id)}

                                <div
                                    class="track-row"
                                    class:playing={isTrackPlaying}
                                    class:paused={isTrackPaused}
                                    class:no-album={!showAlbum}
                                    style="transform: translateY({top}px); height: {height}px;"
                                    role="button"
                                    tabindex="0"
                                    on:click={() => handleTrackToggle(track, item.index)}
                                    on:dblclick={() => handleRowDoubleClick(track, item.index)}
                                    on:contextmenu={(e) => handleTrackContextMenu(track, item.index, e)}
                                    on:keydown={(e) => {
                                        if (e.key === "Enter") handlePlayTrack(track, item.index);
                                    }}
                                >
                                    <!-- Index / Play Button Column -->
                                    <span class="col-num">
                                        {#if isTrackPlaying}
                                            <span class="eq-bars">
                                                <span class="eq-bar"></span>
                                                <span class="eq-bar"></span>
                                                <span class="eq-bar"></span>
                                            </span>
                                        {:else if isTrackPaused}
                                            <span class="paused-icon">
                                                <Icon name="pause" size={14} />
                                            </span>
                                        {:else}
                                            <span class="track-index-text">{item.trackNumber}</span>
                                        {/if}
                                        <button
                                            class="row-hover-play-btn"
                                            aria-label="Play track"
                                            on:click|stopPropagation={() => handleTrackToggle(track, item.index)}
                                        >
                                            <Icon name={isTrackPlaying ? "pause" : "play"} size={14} />
                                        </button>
                                    </span>

                                    <!-- Title & Artist Column -->
                                    <div class="col-title">
                                        <div class="title-line truncate">
                                            <span class="track-title-text" class:highlighted={isTrackPlaying}>
                                                {track.title || $_("player.unknownTitle") || "Untitled"}
                                            </span>
                                            {#if !track.source_type || track.source_type === "local" || track.local_src}
                                                <span class="local-badge" title={$_("album.downloaded") || "Downloaded"}>
                                                    <Icon name="check-circle" size={13} />
                                                </span>
                                            {/if}
                                            {#if track.format}
                                                <span class="format-badge">{track.format.toUpperCase()}</span>
                                            {/if}
                                        </div>
                                        {#if showArtist && (track.artist || (track.artists && track.artists.length > 0))}
                                            <div class="artist-line truncate">
                                                <ArtistLinks
                                                    artist={track.artist || $_("common.unknownArtist") || "Unknown Artist"}
                                                    artists={track.artists}
                                                    chipClass="track-artist-link truncate"
                                                    on:select={(e) => handleArtistClick(e.detail, e)}
                                                />
                                            </div>
                                        {/if}
                                    </div>

                                    <!-- Album Column -->
                                    {#if showAlbum}
                                        <div class="col-album truncate">
                                            {#if track.album}
                                                <button
                                                    class="album-link-btn truncate"
                                                    on:click|stopPropagation={(e) => handleAlbumClick(track.album_id, e)}
                                                >
                                                    {track.album}
                                                </button>
                                            {:else}
                                                <span class="album-muted-text">-</span>
                                            {/if}
                                        </div>
                                    {/if}

                                    <!-- Duration Column -->
                                    <span class="col-duration">
                                        {formatDuration(track.duration || 0)}
                                    </span>

                                    <!-- Actions Column (Like & Context Menu) -->
                                    <div class="col-actions">
                                        <button
                                            class="row-action-btn like-btn"
                                            class:liked={isTrackLiked}
                                            on:click={(e) => handleToggleLike(track, e)}
                                            title={isTrackLiked ? "Unlike" : "Like"}
                                            aria-label={isTrackLiked ? "Unlike" : "Like"}
                                        >
                                            <Icon name="heart" size={16} />
                                        </button>

                                        <button
                                            class="row-action-btn more-btn"
                                            on:click|stopPropagation={(e) => handleTrackContextMenu(track, item.index, e)}
                                            title={$_("common.moreOptions") || "More options"}
                                            aria-label="More options"
                                        >
                                            <Icon name="more-horizontal" size={16} />
                                        </button>
                                    </div>
                                </div>
                            {/if}
                        {/each}
                    </div>
                {:else}
                    <!-- Empty State Slot or Default Empty State -->
                    <slot name="empty">
                        <div class="empty-state">
                            <Icon name="music" size={48} />
                            <h3>{$_("trackList.noTracks") || "No tracks yet"}</h3>
                            <p>{$_("playlist.emptyDescription") || "No songs available in this view."}</p>
                        </div>
                    </slot>
                {/if}
            </div>
        {/if}

        <!-- Slot for Future Downward Scrolling Layers -->
        <div class="bottom-layers">
            <slot name="bottom-layers" />
        </div>
    {/if}
</div>

<!-- Track Metadata Modal -->
{#if metadataModalTrack}
    <MetadataModal
        track={metadataModalTrack}
        onClose={() => (metadataModalTrack = null)}
    />
{/if}

<style>
    /* ──────────────────────────────────────────────
       Layout & Container
       ────────────────────────────────────────────── */
    .detail-view {
        position: relative;
        height: 100%;
        width: 100%;
        overflow-y: auto;
        overflow-x: hidden;
        overscroll-behavior-y: none;
        background: var(--bg-base);
        scroll-behavior: auto;
        display: flex;
        flex-direction: column;
    }

    /* eased fade so the tail is barely visible where the tracklist header sits */
    .hero-backdrop {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        height: 640px;
        background: linear-gradient(
            180deg,
            color-mix(in srgb, var(--backdrop-color) 55%, var(--bg-base)) 0%,
            color-mix(in srgb, var(--backdrop-color) 30%, var(--bg-base)) 40%,
            color-mix(in srgb, var(--backdrop-color) 10%, var(--bg-base)) 75%,
            var(--bg-base) 100%
        );
        pointer-events: none;
        z-index: 0;
    }

    /* ──────────────────────────────────────────────
       Top Navigation Bar
       ────────────────────────────────────────────── */
    .top-nav-bar {
        position: sticky;
        top: 0;
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: var(--spacing-sm) var(--spacing-lg);
        height: 56px;
        z-index: 30;
        transition: background-color var(--transition-fast), border-bottom var(--transition-fast);
    }

    .top-nav-bar.scrolled {
        background: var(--bg-base);
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    }

    .nav-back-btn {
        width: 36px;
        height: 36px;
        border-radius: var(--radius-full);
        background: rgba(0, 0, 0, 0.5);
        color: var(--text-primary);
        display: flex;
        align-items: center;
        justify-content: center;
        border: none;
        cursor: pointer;
        transition: all var(--transition-fast);
        flex-shrink: 0;
    }

    .nav-back-btn:hover {
        background: rgba(0, 0, 0, 0.75);
        transform: scale(1.06);
    }

    .nav-scrolled-info {
        display: flex;
        align-items: center;
        gap: var(--spacing-sm);
        opacity: 0;
        transform: translateY(6px);
        transition: all var(--transition-fast);
        pointer-events: none;
        flex: 1;
        margin: 0 var(--spacing-md);
        min-width: 0;
    }

    .nav-scrolled-info.visible {
        opacity: 1;
        transform: translateY(0);
        pointer-events: auto;
    }

    .nav-scrolled-thumb {
        width: 32px;
        height: 32px;
        border-radius: var(--radius-sm);
        object-fit: cover;
        flex-shrink: 0;
    }

    .nav-scrolled-thumb.circle {
        border-radius: var(--radius-full);
    }

    .nav-scrolled-title {
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-bold);
        color: var(--text-primary);
        max-width: 320px;
    }

    .nav-play-pill {
        width: 32px;
        height: 32px;
        border-radius: var(--radius-full);
        background: var(--accent-primary);
        color: #000;
        display: flex;
        align-items: center;
        justify-content: center;
        border: none;
        cursor: pointer;
        transition: transform var(--transition-fast);
        flex-shrink: 0;
    }

    .nav-play-pill:hover {
        transform: scale(1.1);
    }

    .top-nav-extra {
        display: flex;
        align-items: center;
        gap: var(--spacing-sm);
    }

    /* ──────────────────────────────────────────────
       Hero Section
       ────────────────────────────────────────────── */
    .hero-header {
        position: relative;
        z-index: 1;
        display: flex;
        align-items: flex-end;
        gap: var(--spacing-xl);
        padding: var(--spacing-md) var(--spacing-xl) var(--spacing-lg);
    }

    /* faint dark tint around cover + text, fades out on all edges */
    .hero-header::before {
        content: "";
        position: absolute;
        top: -24px;
        bottom: -24px;
        left: 0;
        right: 0;
        background: linear-gradient(180deg, transparent 0%, rgba(0, 0, 0, 0.22) 55%, transparent 100%);
        pointer-events: none;
        z-index: -1;
    }

    .hero-cover-container {
        position: relative;
        width: 220px;
        height: 220px;
        flex-shrink: 0;
        border-radius: var(--radius-md);
        overflow: hidden;
        box-shadow: 0 16px 36px rgba(0, 0, 0, 0.45);
        background: var(--bg-surface);
    }

    .hero-cover-container.circle {
        border-radius: var(--radius-full);
    }

    .hero-cover-img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }

    .hero-cover-img.circle {
        border-radius: var(--radius-full);
    }

    .hero-cover-placeholder {
        width: 100%;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        background: linear-gradient(135deg, var(--bg-surface) 0%, var(--bg-highlight) 100%);
        color: var(--text-subdued);
    }

    .hero-cover-placeholder.circle {
        border-radius: var(--radius-full);
    }

    .avatar-initials {
        font-size: 4rem;
        font-weight: var(--font-weight-bold);
        color: var(--text-primary);
        text-transform: uppercase;
        user-select: none;
    }

    .cover-hover-overlay {
        position: absolute;
        inset: 0;
        background: rgba(0, 0, 0, 0.6);
        display: flex;
        align-items: center;
        justify-content: center;
        gap: var(--spacing-sm);
        animation: fadeIn 0.15s ease-out;
    }

    .cover-action-btn {
        width: 44px;
        height: 44px;
        border-radius: var(--radius-full);
        background: rgba(255, 255, 255, 0.2);
        color: var(--text-primary);
        border: none;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        transition: all var(--transition-fast);
    }

    .cover-action-btn:hover {
        background: rgba(255, 255, 255, 0.35);
        transform: scale(1.1);
    }

    .cover-action-btn.danger:hover {
        background: var(--accent-error);
        color: #fff;
    }

    .hero-info {
        display: flex;
        flex-direction: column;
        justify-content: flex-end;
        align-items: flex-start;
        flex: 1;
        min-width: 0;
        gap: var(--spacing-xs);
    }

    .hero-type-eyebrow {
        display: inline-flex;
        align-items: center;
        gap: var(--spacing-xs);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-bold);
        text-transform: uppercase;
        letter-spacing: 0.1em;
        color: var(--text-secondary);
    }

    .hero-title {
        font-size: clamp(2rem, 4.5vw, 3.4rem);
        font-weight: 800;
        line-height: 1.1;
        color: var(--text-primary);
        margin: var(--spacing-xs) 0;
        word-break: break-word;
        letter-spacing: -0.02em;
    }

    .hero-title.editable {
        cursor: pointer;
    }

    .hero-title-input {
        font-size: clamp(2rem, 4.5vw, 3.4rem);
        font-weight: 800;
        color: var(--text-primary);
        background: var(--bg-surface);
        border: 2px solid var(--accent-primary);
        border-radius: var(--radius-sm);
        padding: var(--spacing-xs) var(--spacing-sm);
        width: 100%;
        max-width: 600px;
        margin: var(--spacing-xs) 0;
        outline: none;
    }

    .hero-subtitle {
        display: flex;
        align-items: center;
        gap: var(--spacing-sm);
        font-size: var(--font-size-base);
        color: var(--text-primary);
    }

    .hero-stats {
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        gap: var(--spacing-xs);
        font-size: var(--font-size-sm);
        color: var(--text-secondary);
        margin-top: var(--spacing-xs);
    }

    :global(.stat-dot) {
        color: var(--text-subdued);
    }

    /* ──────────────────────────────────────────────
       Action Buttons Bar
       ────────────────────────────────────────────── */
    .action-bar {
        position: relative;
        z-index: 2;
        display: flex;
        align-items: center;
        gap: var(--spacing-md);
        padding: var(--spacing-md) var(--spacing-xl);
    }

    .btn-play-all {
        display: inline-flex;
        align-items: center;
        gap: var(--spacing-sm);
        padding: var(--spacing-sm) var(--spacing-xl);
        background: var(--accent-primary);
        color: #000;
        font-size: var(--font-size-md);
        font-weight: var(--font-weight-bold);
        border-radius: var(--radius-full);
        border: none;
        cursor: pointer;
        transition: transform var(--transition-fast), background-color var(--transition-fast);
        box-shadow: 0 4px 14px rgba(29, 185, 84, 0.35);
    }

    .btn-play-all:hover:not(:disabled) {
        transform: scale(1.05);
        background: var(--accent-hover);
    }

    .btn-play-all:disabled {
        opacity: 0.5;
        cursor: not-allowed;
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

    .btn-action-outline:hover:not(:disabled) {
        border-color: var(--text-primary);
        transform: scale(1.04);
    }

    .btn-action-outline.downloaded {
        border-color: var(--accent-primary);
        color: var(--accent-primary);
        cursor: default;
    }

    .btn-action-outline:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .btn-spinner {
        width: 16px;
        height: 16px;
        border: 2px solid var(--bg-highlight);
        border-top-color: var(--text-primary);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }

    /* ──────────────────────────────────────────────
       Tabs Navigation (e.g. Artist Detail)
       ────────────────────────────────────────────── */
    .tabs-container {
        display: flex;
        gap: var(--spacing-sm);
        padding: 0 var(--spacing-xl) var(--spacing-md);
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    }

    .tab-btn {
        background: transparent;
        border: none;
        color: var(--text-secondary);
        font-size: var(--font-size-base);
        font-weight: var(--font-weight-semibold);
        padding: var(--spacing-sm) var(--spacing-md);
        border-radius: var(--radius-full);
        cursor: pointer;
        transition: all var(--transition-fast);
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
    }

    .tab-btn:hover {
        color: var(--text-primary);
        background: rgba(255, 255, 255, 0.06);
    }

    .tab-btn.active {
        color: var(--text-primary);
        background: var(--bg-highlight);
    }

    .tab-count {
        font-size: var(--font-size-xs);
        background: rgba(255, 255, 255, 0.12);
        padding: 2px 6px;
        border-radius: var(--radius-full);
    }

    .custom-tab-content {
        padding: var(--spacing-md) var(--spacing-xl);
        flex: 1;
    }

    /* ──────────────────────────────────────────────
       Tracklist Section & Virtualized Grid
       ────────────────────────────────────────────── */
    .tracklist-container {
        position: relative;
        z-index: 1;
        padding: 0 var(--spacing-xl) var(--spacing-xl);
        flex: 1;
    }

    .tracklist-header {
        position: sticky;
        top: 56px;
        z-index: 20;
        pointer-events: auto;
        display: grid;
        grid-template-columns: 44px 1fr 1fr 80px 80px;
        align-items: center;
        gap: var(--spacing-md);
        height: 38px;
        padding: 0 var(--spacing-md);
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        color: var(--text-subdued);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.06em;
        background: transparent;
        transition: background-color var(--transition-fast);
    }

    .tracklist-header.stuck {
        background: var(--bg-base);
    }

    .col-header-btn {
        background: transparent;
        border: none;
        color: var(--text-subdued);
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.06em;
        cursor: pointer;
        padding: 0;
        display: flex;
        align-items: center;
        gap: 4px;
        transition: color var(--transition-fast);
        user-select: none;
        min-width: 0;
    }

    .col-header-btn:hover {
        color: var(--text-primary);
    }

    .col-header-btn.active-sort {
        color: var(--accent-primary);
        font-weight: var(--font-weight-bold);
    }

    .col-header-btn.col-num {
        justify-content: center;
    }

    .col-header-btn.col-title {
        justify-content: flex-start;
    }

    .col-header-btn.col-album {
        justify-content: flex-start;
    }

    .col-header-btn.col-duration {
        justify-content: flex-end;
    }

    .sort-arrow {
        font-size: 8px;
        line-height: 1;
        color: var(--accent-primary);
    }

    .tracklist-header.no-album {
        grid-template-columns: 44px 1fr 80px 80px;
    }

    .col-num {
        display: flex;
        align-items: center;
        justify-content: center;
        position: relative;
        text-align: center;
    }

    .col-title {
        min-width: 0;
    }

    .col-album {
        min-width: 0;
    }

    .col-duration {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        text-align: right;
        font-variant-numeric: tabular-nums;
    }

    .col-actions {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: var(--spacing-xs);
    }

    /* Virtualized Spacer */
    .tracks-virtual-spacer {
        position: relative;
        z-index: 0;
        width: 100%;
    }

    /* Track Row */
    .track-row {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        display: grid;
        grid-template-columns: 44px 1fr 1fr 80px 80px;
        align-items: center;
        gap: var(--spacing-md);
        padding: 0 var(--spacing-md);
        border-radius: var(--radius-sm);
        color: var(--text-secondary);
        font-size: var(--font-size-base);
        cursor: pointer;
        user-select: none;
        transition: background-color var(--transition-fast);
        outline: none;
    }

    .track-row.no-album {
        grid-template-columns: 44px 1fr 80px 80px;
    }

    .track-row:hover {
        background-color: var(--bg-surface);
        color: var(--text-primary);
    }

    .track-row.playing {
        color: var(--accent-primary);
    }

    /* disc separator row: same grid as a track row, not interactive */
    .track-row.disc-row {
        cursor: default;
        color: var(--text-secondary);
    }

    .track-row.disc-row:hover {
        background-color: transparent;
        color: var(--text-secondary);
    }

    .disc-title-text {
        font-size: var(--font-size-lg);
        font-weight: var(--font-weight-bold);
        color: var(--text-primary);
    }

    .track-row.playing .track-title-text {
        color: var(--accent-primary);
        font-weight: var(--font-weight-semibold);
    }

    /* Row Hover Play Button & Index */
    .track-index-text {
        color: var(--text-subdued);
        font-size: var(--font-size-sm);
        font-variant-numeric: tabular-nums;
    }

    .row-hover-play-btn {
        display: none;
        position: absolute;
        background: transparent;
        border: none;
        color: var(--text-primary);
        cursor: pointer;
        padding: 0;
    }

    .track-row:hover .track-index-text,
    .track-row:hover .eq-bars,
    .track-row:hover .paused-icon {
        display: none;
    }

    .track-row:hover .row-hover-play-btn {
        display: flex;
        align-items: center;
        justify-content: center;
    }

    /* Equalizer Animation Bars */
    .eq-bars {
        display: flex;
        align-items: flex-end;
        gap: 2px;
        height: 14px;
    }

    .eq-bar {
        width: 3px;
        background-color: var(--accent-primary);
        border-radius: 1px;
        animation: eqBounce 1s ease-in-out infinite alternate;
    }

    .eq-bar:nth-child(1) {
        height: 12px;
        animation-delay: 0.1s;
    }
    .eq-bar:nth-child(2) {
        height: 6px;
        animation-delay: 0.3s;
    }
    .eq-bar:nth-child(3) {
        height: 14px;
        animation-delay: 0.5s;
    }

    @keyframes eqBounce {
        0% {
            height: 3px;
        }
        100% {
            height: 14px;
        }
    }

    .paused-icon {
        color: var(--accent-primary);
        display: flex;
    }

    /* Title line & format tags */
    .title-line {
        display: flex;
        align-items: center;
        gap: var(--spacing-xs);
        font-size: var(--font-size-base);
    }

    .track-title-text {
        color: var(--text-primary);
        font-weight: var(--font-weight-medium);
    }

    .track-title-text.highlighted {
        color: var(--accent-primary);
    }

    .local-badge {
        color: var(--accent-primary);
        display: inline-flex;
        align-items: center;
    }

    .format-badge {
        font-size: 0.65rem;
        font-weight: var(--font-weight-bold);
        background: rgba(255, 255, 255, 0.1);
        color: var(--text-subdued);
        padding: 1px 4px;
        border-radius: 2px;
        letter-spacing: 0.05em;
    }

    .artist-line {
        font-size: var(--font-size-xs);
        color: var(--text-subdued);
        margin-top: 2px;
    }

    .album-link-btn {
        background: transparent;
        border: none;
        color: var(--text-secondary);
        font-size: var(--font-size-sm);
        cursor: pointer;
        padding: 0;
        text-align: left;
    }

    .album-link-btn:hover {
        color: var(--text-primary);
        text-decoration: underline;
    }

    .album-muted-text {
        color: var(--text-tertiary);
    }

    /* Actions Column (Like / More) */
    .row-action-btn {
        background: transparent;
        border: none;
        color: var(--text-subdued);
        width: 30px;
        height: 30px;
        border-radius: var(--radius-full);
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        opacity: 0;
        transition: opacity var(--transition-fast), color var(--transition-fast);
    }

    .row-action-btn:hover {
        color: var(--text-primary);
    }

    .row-action-btn.liked {
        opacity: 1;
        color: var(--accent-primary);
    }

    .track-row:hover .row-action-btn {
        opacity: 1;
    }

    /* ──────────────────────────────────────────────
       Empty State & Loading
       ────────────────────────────────────────────── */
    .state-container,
    .empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: var(--spacing-xl) 0;
        gap: var(--spacing-md);
        color: var(--text-secondary);
        text-align: center;
        flex: 1;
    }

    .loading-spinner {
        width: 36px;
        height: 36px;
        border: 3px solid var(--bg-highlight);
        border-top-color: var(--accent-primary);
        border-radius: 50%;
        animation: spin 1s linear infinite;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    .btn-action {
        padding: var(--spacing-sm) var(--spacing-lg);
        background: var(--bg-highlight);
        color: var(--text-primary);
        border: 1px solid var(--border-color);
        border-radius: var(--radius-full);
        font-size: var(--font-size-base);
        cursor: pointer;
    }

    .btn-action:hover {
        background: var(--bg-press);
    }

    /* ──────────────────────────────────────────────
       Downward Layers Area
       ────────────────────────────────────────────── */
    .bottom-layers {
        position: relative;
        width: 100%;
    }

    /* ──────────────────────────────────────────────
       Scroll-driven collapse (native css)
       unsupported engines keep the class based switch above
       ────────────────────────────────────────────── */
    @supports (animation-timeline: scroll()) {
        .hero-cover-container,
        .hero-info,
        .action-bar,
        .nav-scrolled-info,
        .top-nav-bar {
            animation-timing-function: linear;
            animation-fill-mode: both;
            animation-timeline: scroll(nearest block);
        }

        /* cover shrinks and fades away */
        .hero-cover-container {
            transform-origin: 50% 100%;
            animation-name: dv-cover-shrink;
            animation-range: 0px 240px;
        }

        /* title, subtitle and stats fade out */
        .hero-info {
            animation-name: dv-hero-fade;
            animation-range: 0px 200px;
        }

        /* buttons fade before they slide under the bar */
        .action-bar {
            animation-name: dv-hero-fade;
            animation-range: 100px 300px;
        }

        /* compact strip content fades in as the hero leaves */
        .nav-scrolled-info {
            transition: none;
            animation-name: dv-strip-in;
            animation-range: 140px 240px;
        }

        /* strip gets its bar background */
        .top-nav-bar,
        .top-nav-bar.scrolled {
            transition: none;
            background: transparent;
            border-bottom: 1px solid transparent;
            animation-name: dv-strip-bg;
            animation-range: 120px 200px;
        }
    }

    @keyframes dv-cover-shrink {
        to {
            transform: scale(0.6);
            opacity: 0;
            visibility: hidden;
        }
    }

    @keyframes dv-hero-fade {
        to {
            opacity: 0;
            visibility: hidden;
        }
    }

    @keyframes dv-strip-in {
        from {
            opacity: 0;
            transform: translateY(6px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }

    @keyframes dv-strip-bg {
        to {
            background-color: var(--bg-base);
            border-bottom-color: rgba(255, 255, 255, 0.08);
        }
    }

    /* ──────────────────────────────────────────────
       Responsive Layout Adjustments
       ────────────────────────────────────────────── */
    @media (max-width: 768px) {
        .hero-header {
            flex-direction: column;
            align-items: center;
            text-align: center;
            gap: var(--spacing-md);
            padding: var(--spacing-sm) var(--spacing-md);
        }

        .hero-cover-container {
            width: 160px;
            height: 160px;
        }

        .hero-info {
            align-items: center;
        }

        .hero-title {
            font-size: 1.8rem;
        }

        .action-bar {
            justify-content: center;
            padding: var(--spacing-sm) var(--spacing-md);
        }

        .tracklist-container {
            padding: 0 var(--spacing-sm);
        }

        .tracklist-header {
            grid-template-columns: 36px 1fr 60px 40px;
        }

        .tracklist-header .col-album {
            display: none;
        }

        .track-row {
            grid-template-columns: 36px 1fr 60px 40px;
        }

        .track-row .col-album {
            display: none;
        }

        .row-action-btn.more-btn {
            display: none;
        }
    }
</style>