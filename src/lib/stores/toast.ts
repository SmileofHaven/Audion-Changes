import { writable } from 'svelte/store';

export type ToastType = 'info' | 'success' | 'error' | 'warning';

export interface Toast {
    id: number;
    type: ToastType;
    message: string;
    duration: number;
}

function createToastStore() {
    const { subscribe, update } = writable<Toast[]>([]);
    let counter = 0;

    return {
        subscribe,
        add: (message: string, type: ToastType = 'info', duration: number = 3000) => {
            const id = ++counter;
            const toast: Toast = { id, type, message, duration };

            update(toasts => [...toasts, toast]);

            if (duration > 0) {
                setTimeout(() => {
                    update(toasts => toasts.filter(t => t.id !== id));
                }, duration);
            }
        },
        remove: (id: number) => {
            update(toasts => toasts.filter(t => t.id !== id));
        }
    };
}

export const toasts = createToastStore();

export function addToast(message: string, type: ToastType = 'info', duration: number = 3000) {
    toasts.add(message, type, duration);
}
