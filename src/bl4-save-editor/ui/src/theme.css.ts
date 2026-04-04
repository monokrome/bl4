import { createGlobalTheme } from '@vanilla-extract/css';

export const vars = createGlobalTheme(':root', {
  color: {
    bg: '#0e0618',
    bgSurface: '#1a0a2a',
    bgElevated: '#2d1540',
    accent: '#e07020',
    accentDim: '#a05018',
    accentBright: '#ff8030',
    text: '#d8cce0',
    textDim: '#8070a0',
    textBright: '#f0e8f8',
    border: '#3a2050',
    error: '#e04040',
  },
  font: {
    mono: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
    body: "'Inter', system-ui, -apple-system, sans-serif",
  },
});
