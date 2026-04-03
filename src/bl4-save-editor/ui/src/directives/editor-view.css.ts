import { style } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

export const root = style({
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
});

export const sectionTabs = style({
  display: 'flex',
  gap: 0,
  borderBottom: `1px solid ${vars.color.border}`,
  flexShrink: 0,
  overflowX: 'auto',
});

export const sectionTab = style({
  padding: '10px 18px',
  background: 'none',
  border: 'none',
  borderBottom: '2px solid transparent',
  color: vars.color.textDim,
  fontFamily: vars.font.mono,
  fontSize: '12px',
  fontWeight: 600,
  textTransform: 'uppercase',
  letterSpacing: '1px',
  cursor: 'pointer',
  whiteSpace: 'nowrap',
  transition: 'color 0.1s, border-color 0.1s',
  ':hover': {
    color: vars.color.text,
  },
});

export const sectionTabActive = style({
  color: vars.color.orange,
  borderBottomColor: vars.color.orange,
});

export const sectionContent = style({
  flex: 1,
  padding: '24px',
  color: vars.color.textDim,
  fontFamily: vars.font.mono,
  fontSize: '13px',
});
