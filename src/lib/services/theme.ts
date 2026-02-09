export type ThemeId = 'dark' | 'light';

export function applyTheme(theme: ThemeId): void {
  document.documentElement.setAttribute('data-theme', theme);
}
