import { style } from '@vanilla-extract/css';
import { vars } from '../../theme.css.js';

export const root = style({
  display: 'flex',
  flexDirection: 'column',
  gap: '4px',
});

export const label = style({
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'baseline',
  gap: '8px',
});

export const labelText = style({
  fontFamily: vars.font.mono,
  fontSize: '10px',
  fontWeight: 600,
  color: vars.color.textDim,
  textTransform: 'uppercase',
  letterSpacing: '1px',
});

export const hint = style({
  fontFamily: vars.font.body,
  fontSize: '10px',
  color: vars.color.textDim,
  fontStyle: 'italic',
});

export const control = style({
  display: 'flex',
  alignItems: 'center',
  gap: '6px',
});
