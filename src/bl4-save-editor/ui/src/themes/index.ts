export interface ThemeInfo {
  id: string
  name: string
  css: string
}

const themeModules = import.meta.glob('./*.css', { query: '?inline', import: 'default' })

let cachedThemes: ThemeInfo[] | null = null

export async function loadThemes(): Promise<ThemeInfo[]> {
  if (cachedThemes) return cachedThemes

  const themes: ThemeInfo[] = []
  for (const [path, loader] of Object.entries(themeModules)) {
    const id = path.replace('./', '').replace('.css', '')
    const css = await loader() as string
    const name = extractThemeName(css) || humanize(id)
    themes.push({ id, name, css })
  }

  themes.sort((a, b) => {
    if (a.id === 'vault-hunter') return -1
    if (b.id === 'vault-hunter') return 1
    return a.name.localeCompare(b.name)
  })

  cachedThemes = themes
  return themes
}

function extractThemeName(css: string): string | null {
  const match = css.match(/\/\*\s*@theme-name:\s*(.+?)\s*\*\//)
  return match ? match[1] : null
}

function humanize(id: string): string {
  return id.replace(/[-_]/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

let activeStyleEl: HTMLStyleElement | null = null

export function applyTheme(theme: ThemeInfo | null): void {
  if (activeStyleEl) {
    activeStyleEl.remove()
    activeStyleEl = null
  }

  document.documentElement.removeAttribute('data-theme')

  if (theme && theme.id !== 'vault-hunter') {
    const el = document.createElement('style')
    el.setAttribute('data-theme-id', theme.id)
    el.textContent = theme.css
    document.head.appendChild(el)
    activeStyleEl = el
    document.documentElement.setAttribute('data-theme', theme.id)
  }
}

export function getSavedThemeId(): string {
  try {
    return localStorage.getItem('bl4-theme') || 'vault-hunter'
  } catch {
    return 'vault-hunter'
  }
}

export function saveThemeId(id: string): void {
  try {
    localStorage.setItem('bl4-theme', id)
  } catch {}
}
