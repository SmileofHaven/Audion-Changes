<script lang="ts">
    import { onMount } from "svelte";
    import type { Album, Track } from "$lib/api/tauri";
    import {
        getAlbumsByArtist,
        getTracksByArtist,
        formatDuration,
        getArtistMusicBrainzInfo,
        getSimilarArtistsMb,
        getArtistDiscographyMb,
        type MbArtistInfo,
        type MbSimilarArtist,
        type MbDiscographyItem,
    } from "$lib/api/tauri";
    import { playTracks, currentArtistName, isPlaying } from "$lib/stores/player";
    import { goToArtists, goToArtistDetail } from "$lib/stores/view";
    import AlbumGrid from "./AlbumGrid.svelte";
    import MediaCard from "./MediaCard.svelte";
    import {
        downloadTracks,
        hasDownloadableTracks,
        needsDownloadLocation,
        showDownloadResult,
        type DownloadProgress,
    } from "$lib/services/downloadService";
    import { addToast } from "$lib/stores/toast";
    import { contextMenu } from "$lib/stores/ui";
    import { buildArtistContextMenu } from "$lib/menus/contextMenus";
    import { _ } from "svelte-i18n";
    import Icon from "$lib/components/Icon.svelte";
    import DetailView from "./DetailView.svelte";

    export let artistName: string;

    let albums: Album[] = [];
    let tracks: Track[] = [];
    let loading = true;
    let activeTab: "albums" | "tracks" | "about" = "albums";

    // MusicBrainz artist info
    let mbInfo: MbArtistInfo | null = null;
    let mbLoading = false;
    let mbError: string | null = null;
    let mbFetched = false;

    // Similar artists from MB
    let similarArtists: MbSimilarArtist[] = [];
    let similarLoading = false;
    let similarFetched = false;

    // MusicBrainz discography
    let discography: MbDiscographyItem[] = [];
    let discoLoading = false;
    let discoFetched = false;
    let showAllDiscoTypes = false;
    let failedCovers = new Set<string>();

    const DISCO_PRIMARY = ["album", "single"];

    function discoTypeClass(rt: string): string {
        const l = rt.toLowerCase();
        if (l === "album") return "rt-album";
        if (l === "single") return "rt-single";
        if (l === "ep") return "rt-ep";
        if (l === "live") return "rt-live";
        if (l === "compilation") return "rt-compilation";
        if (l === "soundtrack") return "rt-soundtrack";
        if (l === "remix") return "rt-remix";
        return "rt-other";
    }

    $: filteredDisco = showAllDiscoTypes
        ? discography
        : discography.filter((d) =>
              DISCO_PRIMARY.includes(d.release_type.toLowerCase()),
          );
    $: hiddenDiscoCount = discography.length - filteredDisco.length;

    function handleCoverError(mbid: string) {
        failedCovers.add(mbid);
        failedCovers = failedCovers;
    }

    async function loadMbInfo() {
        if (mbFetched || mbLoading) return;
        mbLoading = true;
        mbError = null;
        try {
            mbInfo = await getArtistMusicBrainzInfo(artistName);
        } catch (e: any) {
            mbError = e?.toString?.() || "Failed to load artist info";
        } finally {
            mbLoading = false;
            mbFetched = true;
        }
    }

    async function loadSimilarArtists() {
        if (similarFetched || similarLoading) return;
        similarLoading = true;
        try {
            similarArtists = await getSimilarArtistsMb(artistName);
        } catch (e) {
            console.warn("[ArtistDetail] Similar artists fetch failed:", e);
        } finally {
            similarLoading = false;
            similarFetched = true;
        }
    }

    async function loadDiscography() {
        if (discoFetched || discoLoading) return;
        discoLoading = true;
        try {
            discography = await getArtistDiscographyMb(artistName);
        } catch (e) {
            console.warn("[ArtistDetail] Discography fetch failed:", e);
        } finally {
            discoLoading = false;
            discoFetched = true;
        }
    }

    function handleAboutTab() {
        loadMbInfo();
        loadSimilarArtists();
        loadDiscography();
    }

    $: totalDuration = tracks.reduce((sum, t) => sum + (t.duration || 0), 0);

    async function loadArtistData() {
        loading = true;
        try {
            const [albumData, trackData] = await Promise.all([
                getAlbumsByArtist(artistName),
                getTracksByArtist(artistName),
            ]);
            albums = albumData;
            tracks = trackData;
        } catch (error) {
            console.error("Failed to load artist:", error);
        } finally {
            loading = false;
        }
    }

    function handlePlayAll() {
        if (tracks.length > 0) {
            playTracks(tracks, 0, {
                type: "artist",
                artistName: artistName,
                displayName: artistName,
            });
        }
    }

    function getArtistInitial(name: string): string {
        return name ? name.charAt(0).toUpperCase() : "A";
    }

    // Download state
    let isDownloading = false;
    let downloadProgress = "";

    // Artist picture handling
    let artistPictureUrl: string | null = null;
    let failedImage = false;

    async function getArtistPicture(name: string) {
        if (!name) return;
        try {
            const plugin = (window as any).tidalSearchPlugin;
            if (!plugin) return;
            const result = await plugin.searchArtistPictureForRPC(name);
            if (result) {
                artistPictureUrl = result;
                failedImage = false;
            }
        } catch (error) {
            console.log("[ArtistDetail] Failed to fetch artist picture:", error);
        }
    }

    onMount(() => {
        loadArtistData();
        getArtistPicture(artistName);
    });

    $: if (artistName) {
        loadArtistData();
        getArtistPicture(artistName);
        mbInfo = null;
        mbFetched = false;
        mbError = null;
        similarArtists = [];
        similarFetched = false;
        discography = [];
        discoFetched = false;
        showAllDiscoTypes = false;
        failedCovers = new Set();
        if (activeTab === "about") {
            handleAboutTab();
        }
    }

    $: hasDownloadable = hasDownloadableTracks(tracks);

    async function handleDownloadAll() {
        if (isDownloading) return;

        if (needsDownloadLocation()) {
            addToast(
                "Please configure a download location in Settings first",
                "error",
            );
            return;
        }

        isDownloading = true;
        downloadProgress = "Starting...";

        try {
            const result = await downloadTracks(
                tracks,
                (progress: DownloadProgress) => {
                    downloadProgress = `${progress.current}/${progress.total}`;
                },
            );

            showDownloadResult(result);
        } catch (error) {
            console.error("Download failed:", error);
            addToast("Download failed unexpectedly", "error");
        } finally {
            isDownloading = false;
            downloadProgress = "";
        }
    }

    function handleContextMenu(e: MouseEvent) {
        e.preventDefault();
        contextMenu.set({
            visible: true,
            x: e.clientX,
            y: e.clientY,
            items: buildArtistContextMenu({
                artist: { name: artistName },
                showPlay: true,
                onPlay: handlePlayAll,
                t: $_,
            }),
        });
    }
</script>

<DetailView
    {loading}
    notFound={!loading && !artistName}
    notFoundTitle={$_("artist.notFound") || "Artist Not Found"}
    notFoundActionLabel={$_("artist.backToArtists") || "Back to Artists"}
    onNotFoundAction={goToArtists}
    title={artistName}
    type={$_("artist.type")}
    typeIcon="user"
    coverSrc={artistPictureUrl && !failedImage ? artistPictureUrl : null}
    coverAlt={artistName}
    coverShape="circle"
    avatarInitials={getArtistInitial(artistName)}
    coverPlaceholderIcon="user"
    onBack={goToArtists}
    backAriaLabel="Back to Artists"
    onHeaderContextMenu={handleContextMenu}
    stats={[
        $_("artist.albums", { values: { count: albums.length } }),
        $_("artist.songs", { values: { count: tracks.length } }),
        formatDuration(totalDuration),
    ]}
    onPlayAll={handlePlayAll}
    playAllDisabled={tracks.length === 0}
    isPlayingThis={$currentArtistName === artistName && $isPlaying}
    {hasDownloadable}
    {isDownloading}
    {downloadProgress}
    onDownloadAll={handleDownloadAll}
    {tracks}
    showAlbum={true}
    playbackContext={{
        type: "artist",
        artistName,
        displayName: artistName,
    }}
    tabs={[
        { id: "albums", label: $_("artist.tabAlbums"), count: albums.length },
        { id: "tracks", label: $_("artist.tabTracks"), count: tracks.length },
        { id: "about", label: $_("artist.tabAbout") },
    ]}
    bind:activeTab
    onTabChange={(tabId) => {
        if (tabId === "about") handleAboutTab();
    }}
>
    <!-- Custom tab content: Albums Grid or About Panel -->
    <svelte:fragment slot="tab-content">
        {#if activeTab === "albums"}
            <div class="albums-tab-pane">
                <AlbumGrid {albums} />
            </div>
        {:else if activeTab === "about"}
            <div class="about-panel">
                {#if mbLoading}
                    <div class="about-loading">
                        <div class="spinner"></div>
                        <span>{$_("artist.lookingUp")}</span>
                    </div>
                {:else if mbError}
                    <p class="about-error">{mbError}</p>
                {:else if mbInfo}
                    {#if mbInfo.genres.length > 0}
                        <section class="about-section">
                            <h3 class="about-heading">{$_("artist.genres")}</h3>
                            <div class="genre-pills">
                                {#each mbInfo.genres as genre}
                                    <span class="genre-pill">{genre}</span>
                                {/each}
                            </div>
                        </section>
                    {/if}

                    {#if mbInfo.bio}
                        <section class="about-section">
                            <h3 class="about-heading">{$_("artist.about")}</h3>
                            <p class="about-bio">{mbInfo.bio}</p>
                        </section>
                    {/if}

                    {#if mbInfo.wikipedia_url}
                        <a
                            class="wiki-link"
                            href={mbInfo.wikipedia_url}
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            <Icon name="external-link" size={16} />
                            {$_("artist.readOnWikipedia")}
                        </a>
                    {/if}

                    {#if mbInfo.disambiguation}
                        <p class="about-disambiguation">
                            ({mbInfo.disambiguation})
                        </p>
                    {/if}

                    <p class="mb-attribution">{$_("artist.dataFromMusicBrainz")}</p>
                {:else}
                    <div class="about-empty">
                        <p>{$_("artist.noInfoFound")}</p>
                    </div>
                {/if}

                <!-- Similar / Related Artists -->
                <section class="about-section">
                    <h3 class="about-heading">{$_("artist.relatedArtists")}</h3>
                    {#if similarLoading}
                        <div class="about-loading">
                            <div class="spinner"></div>
                            <span>{$_("artist.lookingUpRelated")}</span>
                        </div>
                    {:else if similarArtists.length > 0}
                        <div class="similar-list">
                            {#each similarArtists as a}
                                <MediaCard
                                    variant="round"
                                    primaryText={a.name}
                                    secondaryText={a.relation_type}
                                    isPinned={false}
                                    on:click={() => goToArtistDetail(a.name)}
                                >
                                    <svelte:fragment slot="cover">
                                        <div class="artist-initial-sm">
                                            {a.name.charAt(0).toUpperCase()}
                                        </div>
                                    </svelte:fragment>
                                    <svelte:fragment slot="extra-info">
                                        {#if a.in_library}
                                            <span
                                                class="in-library-dot"
                                                title="In your library"
                                                >•</span
                                            >
                                        {/if}
                                    </svelte:fragment>
                                </MediaCard>
                            {/each}
                        </div>
                    {:else if similarFetched}
                        <p class="about-empty-sm">
                            {$_("artist.noRelatedFound")}
                        </p>
                    {/if}
                </section>

                <!-- Discography -->
                <section class="about-section">
                    <div class="disco-section-header">
                        <h3 class="about-heading">
                            {$_("artist.discography")}
                        </h3>
                        {#if discoFetched && (hiddenDiscoCount > 0 || showAllDiscoTypes)}
                            <button
                                class="disco-toggle-btn"
                                on:click={() => (showAllDiscoTypes = !showAllDiscoTypes)}
                            >
                                {showAllDiscoTypes
                                    ? $_("artist.albumsAndSingles")
                                    : $_("artist.showAll", { values: { count: discography.length } })}
                            </button>
                        {/if}
                    </div>
                    {#if discoLoading}
                        <div class="about-loading">
                            <div class="spinner"></div>
                            <span>{$_("artist.loadingDiscography")}</span>
                        </div>
                    {:else if filteredDisco.length > 0}
                        <div class="disco-grid">
                            {#each filteredDisco as item (item.mbid)}
                                <MediaCard
                                    primaryText={item.title}
                                    secondaryText={item.year
                                        ? `${item.release_type} • ${item.year}`
                                        : item.release_type}
                                    ariaLabel={item.title}
                                >
                                    <svelte:fragment slot="cover">
                                        {#if !failedCovers.has(item.mbid)}
                                            <img
                                                class="disco-cover"
                                                src={item.cover_url}
                                                alt={item.title}
                                                loading="lazy"
                                                on:error={() => handleCoverError(item.mbid)}
                                            />
                                        {:else}
                                            <div
                                                class="disco-cover-placeholder {discoTypeClass(item.release_type)}"
                                            >
                                                {#if item.release_type.toLowerCase() === "album"}
                                                    <Icon name="disc" size={28} />
                                                {:else if item.release_type.toLowerCase() === "single"}
                                                    <Icon name="music" size={28} />
                                                {:else if item.release_type.toLowerCase() === "ep"}
                                                    <Icon name="disc" size={28} />
                                                {:else if item.release_type.toLowerCase() === "live"}
                                                    <Icon name="mic-2" size={28} />
                                                {:else if item.release_type.toLowerCase() === "compilation"}
                                                    <Icon name="library" size={28} />
                                                {:else if item.release_type.toLowerCase() === "soundtrack"}
                                                    <Icon name="disc-3" size={28} />
                                                {:else}
                                                    <Icon name="disc" size={28} />
                                                {/if}
                                            </div>
                                        {/if}
                                    </svelte:fragment>
                                </MediaCard>
                            {/each}
                        </div>
                    {:else if discoFetched}
                        <p class="about-empty-sm">{$_("artist.noDiscography")}</p>
                    {/if}
                </section>
            </div>
        {/if}
    </svelte:fragment>
</DetailView>

<style>
    .albums-tab-pane {
        width: 100%;
        min-height: 400px;
    }

    .about-panel {
        display: flex;
        flex-direction: column;
        gap: var(--spacing-xl);
        max-width: 900px;
        padding-bottom: var(--spacing-xl);
    }

    .about-loading {
        display: flex;
        align-items: center;
        gap: var(--spacing-md);
        color: var(--text-secondary);
        font-size: var(--font-size-sm);
        padding: var(--spacing-xl) 0;
    }

    .spinner {
        width: 24px;
        height: 24px;
        border: 2px solid var(--bg-highlight);
        border-top-color: var(--accent-primary);
        border-radius: 50%;
        animation: spin 1s linear infinite;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    .about-error {
        color: var(--color-error, #f87171);
        font-size: var(--font-size-base);
    }

    .about-section {
        display: flex;
        flex-direction: column;
        gap: var(--spacing-sm);
    }

    .about-heading {
        font-size: 0.75rem;
        font-weight: var(--font-weight-bold);
        text-transform: uppercase;
        letter-spacing: 2px;
        color: var(--text-subdued);
        margin: 0;
    }

    .genre-pills {
        display: flex;
        flex-wrap: wrap;
        gap: var(--spacing-sm);
    }

    .genre-pill {
        background: rgba(var(--accent-primary-rgb, 30 215 96) / 0.12);
        border: 1px solid rgba(var(--accent-primary-rgb, 30 215 96) / 0.3);
        color: var(--accent-primary);
        padding: var(--spacing-xs) var(--spacing-md);
        border-radius: var(--radius-full);
        font-size: 0.8rem;
        font-weight: var(--font-weight-semibold);
    }

    .about-bio {
        font-size: 0.9rem;
        line-height: 1.7;
        color: var(--text-secondary);
        margin: 0;
    }

    .wiki-link {
        display: inline-flex;
        align-items: center;
        gap: var(--spacing-xs);
        font-size: 0.8rem;
        font-weight: var(--font-weight-semibold);
        color: var(--text-secondary);
        border: 1px solid var(--border-color);
        border-radius: var(--radius-full);
        padding: var(--spacing-xs) var(--spacing-md);
        width: fit-content;
        text-decoration: none;
        transition: all var(--transition-fast);
    }

    .wiki-link:hover {
        color: var(--text-primary);
        border-color: var(--text-primary);
    }

    .about-disambiguation {
        font-size: 0.8rem;
        color: var(--text-subdued);
        font-style: italic;
        margin: 0;
    }

    .mb-attribution {
        font-size: 0.65rem;
        color: var(--text-subdued);
        text-transform: uppercase;
        letter-spacing: 1px;
        opacity: 0.5;
        margin: 0;
    }

    .about-empty {
        color: var(--text-secondary);
        font-size: var(--font-size-base);
        padding: var(--spacing-xl) 0;
    }

    .about-empty-sm {
        font-size: 0.8rem;
        color: var(--text-subdued);
        margin: 0;
    }

    /* ── Similar artists ── */
    .similar-list {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
        gap: var(--spacing-sm);
    }

    .artist-initial-sm {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 2rem;
        font-weight: var(--font-weight-bold);
        color: var(--text-primary);
        background: var(--bg-highlight);
    }

    .in-library-dot {
        color: var(--accent-primary);
        font-size: 1.2rem;
        line-height: 1;
        flex-shrink: 0;
    }

    /* ── Discography ── */
    .disco-section-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: var(--spacing-sm);
    }

    .disco-section-header .about-heading {
        margin-bottom: 0;
    }

    .disco-toggle-btn {
        background: var(--bg-highlight);
        color: var(--text-secondary);
        border: 1px solid var(--border-color);
        border-radius: var(--radius-full);
        padding: var(--spacing-xs) var(--spacing-md);
        font-size: 0.72rem;
        font-weight: var(--font-weight-semibold);
        cursor: pointer;
        transition: all var(--transition-fast);
        white-space: nowrap;
    }

    .disco-toggle-btn:hover {
        background: var(--accent-subtle);
        color: var(--accent-primary);
        border-color: var(--accent-primary);
    }

    .disco-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
        gap: var(--spacing-sm);
    }

    .disco-cover {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }

    .disco-cover-placeholder {
        width: 100%;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--text-subdued);
        background: var(--bg-surface);
    }

    .disco-cover-placeholder.rt-album {
        color: #3b82f6;
        background: rgba(59, 130, 246, 0.08);
    }
    .disco-cover-placeholder.rt-single {
        color: #22c55e;
        background: rgba(34, 197, 94, 0.08);
    }
    .disco-cover-placeholder.rt-ep {
        color: #a855f7;
        background: rgba(168, 85, 247, 0.08);
    }
    .disco-cover-placeholder.rt-live {
        color: #ef4444;
        background: rgba(239, 68, 68, 0.08);
    }
    .disco-cover-placeholder.rt-compilation {
        color: #f59e0b;
        background: rgba(245, 158, 11, 0.08);
    }
    .disco-cover-placeholder.rt-soundtrack {
        color: #ec4899;
        background: rgba(236, 72, 153, 0.08);
    }
    .disco-cover-placeholder.rt-remix {
        color: #06b6d4;
        background: rgba(6, 182, 212, 0.08);
    }
    .disco-cover-placeholder.rt-other {
        color: #6b7280;
        background: rgba(107, 114, 128, 0.08);
    }
</style>
