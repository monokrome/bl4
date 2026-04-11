import { style } from '@vanilla-extract/css';
import { vars } from '../../theme.css.js';

export const root = style({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: '14px',
  height: '14px',
  color: vars.color.accentBright,
  fontSize: '10px',
  cursor: 'pointer',
  userSelect: 'none',
  flexShrink: 0,
});
