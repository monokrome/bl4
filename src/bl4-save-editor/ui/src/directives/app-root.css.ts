import { style, globalStyle } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

export const root = style({
  display: 'flex',
  flexDirection: 'column',
  height: '100vh',
});

export const mainArea = style({
  display: 'flex',
  flex: 1,
  overflow: 'hidden',
});

export const content = style({
  flex: 1,
  overflowY: 'auto',
  padding: '24px',
});

export const drawer = style({
  width: 0,
  overflow: 'hidden',
  background: vars.color.bgSurface,
  borderLeft: `1px solid ${vars.color.border}`,
  transition: 'width 0.2s ease',
});

export const drawerOpen = style({});

globalStyle(`${drawerOpen} ${drawer}`, {
  width: '320px',
});

export const drawerInner = style({
  width: '320px',
  padding: '16px',
  height: '100%',
  display: 'flex',
  flexDirection: 'column',
});

export const drawerTitle = style({
  fontFamily: vars.font.mono,
  fontSize: '12px',
  fontWeight: 600,
  color: vars.color.textDim,
  textTransform: 'uppercase',
  letterSpacing: '1px',
  marginBottom: '16px',
});
