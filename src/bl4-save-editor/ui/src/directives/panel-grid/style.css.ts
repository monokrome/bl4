import { style } from '@vanilla-extract/css';

/// Responsive grid of panels that wraps based on available width.
export const root = style({
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
  gap: '12px',
  padding: '16px',
});
