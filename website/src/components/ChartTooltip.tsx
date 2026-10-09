import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';

export type ChartCursor = { x: number; y: number; container?: Element | null };

export function ChartTooltip({
  cursor,
  children,
}: {
  cursor: ChartCursor | null;
  children: ReactNode;
}) {
  if (!cursor) return null;
  const left = cursor.x > window.innerWidth - 260;
  const above = cursor.y > window.innerHeight - 150;
  return createPortal(
    <div
      className="cursor-chart-tooltip"
      role="status"
      style={{
        left: cursor.x + (left ? -12 : 12),
        top: cursor.y + (above ? -12 : 12),
        transform: `translate(${left ? '-100%' : '0'}, ${above ? '-100%' : '0'})`,
      }}
    >
      {children}
    </div>,
    cursor.container || document.body,
  );
}
