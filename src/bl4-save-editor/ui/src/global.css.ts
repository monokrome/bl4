import { globalStyle } from '@vanilla-extract/css';
import { vars } from './theme.css.js';

globalStyle('*', {
  margin: 0,
  padding: 0,
  boxSizing: 'border-box',
});

globalStyle('html, body', {
  height: '100%',
  background: vars.color.bgDeep,
  color: vars.color.text,
  fontFamily: vars.font.body,
  fontSize: '14px',
  overflow: 'hidden',
});
