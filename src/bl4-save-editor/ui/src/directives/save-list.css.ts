import { style } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

export const root = style({
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
});

export const collapsed = style({
  gap: 0,
});

export const saveRow = style({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  padding: '12px 16px',
  background: vars.color.bgSurface,
  cursor: 'pointer',
  transition: 'background 0.1s',
  ':hover': {
    background: vars.color.bgElevated,
  },
});

export const saveName = style({
  fontFamily: vars.font.mono,
  fontSize: '14px',
  fontWeight: 600,
  color: vars.color.textBright,
  textTransform: 'uppercase',
  letterSpacing: '1px',
});

export const saveHint = style({
  fontSize: '12px',
  color: vars.color.textDim,
  fontFamily: vars.font.mono,
});
