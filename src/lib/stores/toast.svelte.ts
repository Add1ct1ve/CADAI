export interface ToastItem {
  id: string;
  message: string;
  type: 'success' | 'error' | 'warning' | 'info';
  duration: number;
}

let toasts = $state<ToastItem[]>([]);
let nextId = 0;

export function getToastStore() {
  return {
    get toasts() {
      return toasts;
    },
    add(message: string, type: ToastItem['type'] = 'info', duration?: number) {
      const id = String(++nextId);
      const dur = duration ?? (type === 'error' ? 5000 : 3000);
      toasts = [...toasts, { id, message, type, duration: dur }];
      setTimeout(() => {
        this.remove(id);
      }, dur);
      return id;
    },
    remove(id: string) {
      toasts = toasts.filter((t) => t.id !== id);
    },
    success(message: string, duration?: number) {
      return this.add(message, 'success', duration);
    },
    error(message: string, duration?: number) {
      return this.add(message, 'error', duration);
    },
    warning(message: string, duration?: number) {
      return this.add(message, 'warning', duration);
    },
    info(message: string, duration?: number) {
      return this.add(message, 'info', duration);
    },
  };
}
