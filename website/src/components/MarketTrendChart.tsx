import { useId, useState } from 'react';
import '../market-trend.css';

export type TrendPoint = { timestamp: string; value: number; sampleSize: number };

const date = (timestamp: number) =>
  new Date(timestamp).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: '2-digit',
  });

export function MarketTrendChart({
  points,
  title,
  valueLabel,
  tickLabel,
  baseline,
  minimumPadding = 1,
}: {
  points: TrendPoint[];
  title: string;
  valueLabel: (value: number) => string;
  tickLabel: (value: number) => string;
  baseline?: number;
  minimumPadding?: number;
}) {
  const id = useId();
  const [active, setActive] = useState<number | null>(null);
  if (points.length < 2) return null;

  const times = points.map((point) => Date.parse(point.timestamp));
  const start = times[0];
  const end = times.at(-1)!;
  const values = points.map((point) => point.value);
  const lowest = Math.min(...values, baseline ?? Infinity);
  const highest = Math.max(...values, baseline ?? -Infinity);
  const padding = Math.max(minimumPadding, (highest - lowest) * 0.15);
  const low = baseline === undefined ? Math.max(0, lowest - padding) : lowest - padding;
  const high = Math.max(low + minimumPadding * 2, highest + padding);
  const y = (value: number) => 190 - ((value - low) / (high - low)) * 160;
  const x = (index: number) =>
    end === start ? 352 : 64 + ((times[index] - start) / (end - start)) * 576;
  const path = points.slice(1).reduce(
    (line, point, index) => {
      const previous = points[index];
      const middle = (x(index) + x(index + 1)) / 2;
      return `${line} C ${middle} ${y(previous.value)}, ${middle} ${y(point.value)}, ${x(index + 1)} ${y(point.value)}`;
    },
    `M ${x(0)} ${y(points[0].value)}`,
  );
  const area = `${path} L ${x(points.length - 1)} 190 L ${x(0)} 190 Z`;
  const activePoint = active === null ? null : points[active];
  const activeX = active === null ? 0 : x(active);
  const activeY = activePoint ? y(activePoint.value) : 0;

  return (
    <div className="history-chart market-trend-chart" onPointerLeave={() => setActive(null)}>
      <svg
        viewBox="0 0 670 232"
        role="img"
        aria-labelledby={id}
        onPointerMove={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          const target = ((event.clientX - rect.left) / rect.width) * 670;
          setActive(
            times.reduce(
              (best, _, index) =>
                Math.abs(x(index) - target) < Math.abs(x(best) - target) ? index : best,
              0,
            ),
          );
        }}
      >
        <title id={id}>{title}</title>
        <defs>
          <linearGradient id={`${id}-fill`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity=".3" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity=".02" />
          </linearGradient>
        </defs>
        <path d={area} fill={`url(#${id}-fill)`} />
        {[0, 1, 2, 3, 4].map((tick) => {
          const value = low + ((high - low) * tick) / 4;
          return (
            <g key={tick}>
              <line x1="64" x2="640" y1={y(value)} y2={y(value)} className="chart-grid" />
              <text x="54" y={y(value) + 4} textAnchor="end">
                {tickLabel(value)}
              </text>
            </g>
          );
        })}
        <path d={path} className="chart-series-line" />
        {activePoint && (
          <>
            <line x1={activeX} x2={activeX} y1="24" y2="190" className="chart-cursor" />
            <circle cx={activeX} cy={activeY} r="4.5" className="chart-dot" />
          </>
        )}
        {points.map((point, index) => (
          <circle
            key={`${point.timestamp}-${index}`}
            cx={x(index)}
            cy={y(point.value)}
            r="10"
            className="chart-hit-target"
            tabIndex={0}
            aria-label={`${date(times[index])}: ${valueLabel(point.value)}, ${point.sampleSize} offers observed`}
            onFocus={() => setActive(index)}
            onBlur={() => setActive(null)}
          />
        ))}
        {(start === end ? [0.5] : [0, 0.5, 1]).map((tick) => (
          <text
            key={tick}
            x={64 + 576 * tick}
            y="218"
            textAnchor={tick === 0 ? 'start' : tick === 1 ? 'end' : 'middle'}
          >
            {date(start + (end - start) * tick)}
          </text>
        ))}
      </svg>
      {activePoint && (
        <div
          className={`chart-tooltip ${activeX > 500 ? 'chart-tooltip-left' : ''}`}
          style={{ left: `${(activeX / 670) * 100}%`, top: `${(activeY / 232) * 100}%` }}
          role="status"
        >
          <span>{date(times[active!])}</span>
          <strong>{valueLabel(activePoint.value)}</strong>
          <small>{activePoint.sampleSize.toLocaleString()} offers in this observation</small>
        </div>
      )}
    </div>
  );
}
