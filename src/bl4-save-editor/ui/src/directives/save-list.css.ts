import { style, globalStyle } from '@vanilla-extract/css';
import { vars } from '../theme.css.js';

const SKEW = 20;

export const root = style({
  display: 'grid',
  gridTemplateColumns: 'auto 1fr auto',
  rowGap: '2px',
});

export const collapsed = style({
  gridTemplateColumns: 'auto',
  alignContent: 'start',
});

export const saveRow = style({
  display: 'contents',
  cursor: 'pointer',
});

const segmentBase = style({
  display: 'flex',
  alignItems: 'center',
  height: '48px',
  fontFamily: vars.font.mono,
  letterSpacing: '0.5px',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  cursor: 'pointer',
});

export const segName = style([segmentBase, {
  fontWeight: 600,
  fontSize: '16px',
  color: vars.color.bg,
  textTransform: 'uppercase',
  background: vars.color.accent,
  padding: `0 ${SKEW + 4}px 0 8px`,
  clipPath: `polygon(0 0, 100% 0, calc(100% - ${SKEW}px) 100%, 0 100%)`,
}]);

export const segBlade = style({
  height: '48px',
  marginLeft: `-${SKEW}px`,
  width: `${SKEW + 8}px`,
  background: '#802050',
  clipPath: `polygon(${SKEW}px 0, ${SKEW + 8}px 0, 8px 100%, 0 100%)`,
  opacity: 0,
  transition: 'opacity 0.15s ease',
  cursor: 'pointer',
});

export const segAttr = style([segmentBase, {
  minWidth: 0,
  fontSize: '13px',
  color: vars.color.textDim,
  background: vars.color.bgElevated,
  padding: `0 ${SKEW + 12}px 0 ${SKEW + 12}px`,
  marginLeft: `-${SKEW}px`,
  clipPath: `polygon(${SKEW}px 0, 100% 0, calc(100% - ${SKEW}px) 100%, 0 100%)`,
}]);

export const segLevel = style([segmentBase, {
  flexDirection: 'column',
  alignItems: 'flex-end',
  justifyContent: 'center',
  fontSize: '13px',
  color: vars.color.text,
  background: vars.color.bgSurface,
  padding: `0 20px 0 ${SKEW + 12}px`,
  marginLeft: `-${SKEW}px`,
  clipPath: `polygon(${SKEW}px 0, 100% 0, 100% 100%, 0 100%)`,
}]);

export const segFileName = style({
  fontSize: '10px',
  color: vars.color.textDim,
  letterSpacing: '0.5px',
});

/* Hover: show blade */
globalStyle(`${saveRow}:hover ${segBlade}`, {
  opacity: 1,
});

/* Collapsed mode: scaleX + hide other segments */
globalStyle(`${collapsed} ${segName}`, {
  transition: 'transform 0.15s ease',
  transformOrigin: 'left center',
});

globalStyle(`${collapsed} ${segName}:hover`, {
  transform: 'scaleX(1.08)',
});

globalStyle(`${collapsed} ${segAttr}`, { display: 'none' });
globalStyle(`${collapsed} ${segLevel}`, { display: 'none' });
