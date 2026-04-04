import { style } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

export const root = style({
  display: 'flex',
  alignItems: 'flex-start',
  justifyContent: 'space-between',
  height: '42px',
  padding: '0 1em',
  background: vars.color.accent,
  flexShrink: 0,
  overflow: 'visible',
  position: 'relative',
  zIndex: 10,
  marginBottom: '2px',
});

export const title = style({
  fontFamily: vars.font.mono,
  fontSize: '64px',
  fontWeight: 700,
  color: vars.color.bg,
  letterSpacing: '5px',
  textTransform: 'uppercase',
  lineHeight: 0.7
});

export const toggle = style({
  width: '36px',
  height: '36px',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  background: 'none',
  border: 'none',
  color: vars.color.bg,
  fontFamily: vars.font.mono,
  fontSize: '18px',
  cursor: 'pointer',
  marginTop: '2px',
  transition: 'color 0.15s, border-color 0.15s',
  ':hover': {
    color: vars.color.bgSurface,
    borderColor: vars.color.bgSurface,
  },
});
