import { style, keyframes } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

export const root = style({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  height: '100%',
  gap: '24px',
});

export const openPrompt = style({
  fontFamily: vars.font.mono,
  fontSize: '13px',
  color: vars.color.textDim,
  textTransform: 'uppercase',
  letterSpacing: '1px',
});

export const openActions = style({
  display: 'flex',
  gap: '12px',
});

const buttonBase = style({
  padding: '12px 24px',
  background: vars.color.bgElevated,
  border: `1px solid ${vars.color.border}`,
  color: vars.color.text,
  fontFamily: vars.font.mono,
  fontSize: '13px',
  fontWeight: 600,
  textTransform: 'uppercase',
  letterSpacing: '1px',
  cursor: 'pointer',
  transition: 'background 0.15s, border-color 0.15s, color 0.15s',
  ':hover': {
    background: vars.color.accentDim,
    borderColor: vars.color.accent,
    color: vars.color.textBright,
  },
});

export const openDir = buttonBase;
export const openFile = buttonBase;

export const steamIdSection = style({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: '8px',
});

export const steamIdLabel = style({
  fontSize: '11px',
  color: vars.color.textDim,
  textTransform: 'uppercase',
  letterSpacing: '1px',
});

export const steamIdInput = style({
  width: '280px',
  padding: '8px 12px',
  background: vars.color.bg,
  border: `1px solid ${vars.color.border}`,
  color: vars.color.text,
  fontFamily: vars.font.mono,
  fontSize: '13px',
  textAlign: 'center',
  outline: 'none',
  ':focus': {
    borderColor: vars.color.accentDim,
  },
});

export const error = style({
  color: vars.color.error,
  fontSize: '12px',
  fontFamily: vars.font.mono,
  maxWidth: '400px',
  textAlign: 'center',
});

const pulse = keyframes({
  '0%, 100%': { opacity: 0.3 },
  '50%': { opacity: 1 },
});

export const loading = style({
  display: 'none',
  alignItems: 'center',
  gap: '8px',
  color: vars.color.textDim,
  fontFamily: vars.font.mono,
  fontSize: '12px',
});

export const loadingDot = style({
  width: '6px',
  height: '6px',
  background: vars.color.accent,
  animation: `${pulse} 1s ease-in-out infinite`,
});
