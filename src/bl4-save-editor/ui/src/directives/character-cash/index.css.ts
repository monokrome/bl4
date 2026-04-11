import { style } from '@vanilla-extract/css';
import { vars } from '../../theme.css.js';

export const input = style({
  flex: 1,
  minWidth: 0,
  padding: '6px 10px',
  background: vars.color.bg,
  border: `1px solid ${vars.color.border}`,
  borderRadius: '3px',
  color: vars.color.text,
  fontFamily: vars.font.mono,
  fontSize: '13px',
  textAlign: 'right',
  outline: 'none',
  transition: 'border-color 0.1s',
  ':focus': {
    borderColor: vars.color.accentDim,
  },
});

export const dirty = style({
  borderColor: vars.color.accent,
  borderLeftWidth: '3px',
  paddingLeft: '8px',
});

export const marker = style({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: '14px',
  height: '14px',
  color: vars.color.accentBright,
  fontSize: '10px',
  flexShrink: 0,
});
