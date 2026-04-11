import { style } from '@vanilla-extract/css';
import { vars } from '../../theme.css.js';

export const root = style({
  background: vars.color.bgSurface,
  border: `1px solid ${vars.color.border}`,
  borderRadius: '4px',
  padding: '14px 16px',
  display: 'flex',
  flexDirection: 'column',
  gap: '10px',
  minWidth: 0,
});

export const header = style({
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
  paddingBottom: '8px',
  borderBottom: `1px solid ${vars.color.border}`,
});

export const title = style({
  fontFamily: vars.font.mono,
  fontSize: '11px',
  fontWeight: 600,
  color: vars.color.accent,
  textTransform: 'uppercase',
  letterSpacing: '1.5px',
  margin: 0,
});

export const subtitle = style({
  fontFamily: vars.font.body,
  fontSize: '11px',
  color: vars.color.textDim,
  margin: 0,
});

export const body = style({
  display: 'flex',
  flexDirection: 'column',
  gap: '10px',
});
