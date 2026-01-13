import { writable } from 'svelte/store';

interface UpdateState {
    hasUpdate: boolean;
    latestRelease: any | null;
    checking: boolean;
    error: string | null;
}

function createUpdateStore() {
    const { subscribe, set, update } = writable<UpdateState>({
        hasUpdate: false,
        latestRelease: null,
        checking: false,
        error: null
    });

    return {
        subscribe,
        checkUpdate: async () => {
            update(s => ({ ...s, checking: true, error: null }));
            try {
                const response = await fetch("https://api.github.com/repos/dupitydumb/Audion/releases/latest");
                if (response.ok) {
                    const data = await response.json();

                    // transform tag to version number (remove 'v' prefix if present)
                    const latestVersion = data.tag_name.replace(/^v/, '');
                    const currentVersion = __APP_VERSION__.replace(/^v/, '');

                    const hasUpdate = latestVersion !== currentVersion;

                    set({
                        hasUpdate,
                        latestRelease: data,
                        checking: false,
                        error: null
                    });
                } else {
                    throw new Error(`Failed to fetch release: ${response.statusText}`);
                }
            } catch (error) {
                console.error("Failed to check version:", error);
                update(s => ({
                    ...s,
                    checking: false,
                    error: error instanceof Error ? error.message : String(error)
                }));
            }
        }
    };
}

export const updates = createUpdateStore();
