import { style, globalStyle } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

export const root = style({
  display: 'flex',
  flexDirection: 'column',
  height: '100vh',
  position: 'relative',
});

export const mainArea = style({
  flex: 1,
  overflow: 'hidden',
});

export const content = style({
  width: '100%',
  height: '100%',
  overflowY: 'auto',
});

export const contentWithEditor = style({
  display: 'flex',
  flexDirection: 'row',
});

export const drawer = style({
  position: 'absolute',
  top: 0,
  right: 0,
  height: '100%',
  width: 0,
  overflow: 'hidden',
  background: vars.color.bgSurface,
  borderLeft: `1px solid ${vars.color.border}`,
  transition: 'width 0.2s ease',
  zIndex: 100,
});

export const drawerOpen = style({
  width: '320px',
});

export const drawerInner = style({
  width: '320px',
  padding: '16px',
  height: '100%',
  display: 'flex',
  flexDirection: 'column',
});

export const drawerHeader = style({
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'center',
  marginBottom: '16px',
});

export const drawerTitle = style({
  fontFamily: vars.font.mono,
  fontSize: '12px',
  fontWeight: 600,
  color: vars.color.textDim,
  textTransform: 'uppercase',
  letterSpacing: '1px',
});

export const drawerClose = style({
  background: 'none',
  border: 'none',
  color: vars.color.textDim,
  fontSize: '20px',
  cursor: 'pointer',
  padding: '4px 8px',
  lineHeight: 1,
  ':hover': {
    color: vars.color.text,
  },
});
