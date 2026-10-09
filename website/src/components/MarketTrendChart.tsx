import { useEffect, useId, useRef, useState } from 'react';
import '../market-trend.css';
import { ChartTooltip, type ChartCursor } from './ChartTooltip';
import { trendBounds } from '../lib/chartScale';

export type TrendPoint = {
  timestamp: string;
  value: number;
  sampleSize: number;
  breakBefore?: boolean;
};

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
  colorBySign = false,
  maximumValue,
}: {
  points: TrendPoint[];
  title: string;
  valueLabel: (value: number) => string;
  tickLabel: (value: number) => string;
  baseline?: number;
  minimumPadding?: number;
  colorBySign?: boolean;
  maximumValue?: number;
}) {
  const id = useId();
  const [active, setActive] = useState<number | null>(null);
  const [cursor, setCursor] = useState<ChartCursor | null>(null);
  const container = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(670);
  useEffect(() => {
    const element = container.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) =>
      setWidth(Math.max(200, entry.contentRect.width)),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [points.length > 1]);
  if (points.length < 2) return null;

  const times = points.map((point) => Date.parse(point.timestamp));
  const start = times[0];
  const end = times.at(-1)!;
  const values = points.map((point) => point.value);
  const { low, high } = trendBounds(values, { baseline, minimumPadding, maximumValue });
  const ticks = [0, 1, 2, 3, 4].map((tick) => low + ((high - low) * tick) / 4);
  const left = Math.max(46, ...ticks.map((value) => tickLabel(value).length * 6.5 + 12));
  const right = width - 14;
  const y = (value: number) => 190 - ((value - low) / (high - low)) * 160;
  const zeroOffset = Math.max(0, Math.min(1, (y(baseline ?? 0) - 30) / 160));
  const seriesColor = colorBySign ? `url(#${id}-sign)` : 'var(--accent)';
  const areaBaseline = colorBySign ? y(baseline ?? 0) : 190;
  const x = (index: number) =>
    end === start
      ? (left + right) / 2
      : left + ((times[index] - start) / (end - start)) * (right - left);
  const segments: number[][] = [];
  points.forEach((point, index) => {
    if (!segments.length || point.breakBefore) segments.push([]);
    segments.at(-1)!.push(index);
  });
  const paths = segments.map((indices) => ({
    line: indices
      .map((index, position) => `${position ? 'L' : 'M'} ${x(index)} ${y(points[index].value)}`)
      .join(' '),
    start: indices[0],
    end: indices.at(-1)!,
  }));
  const activePoint = active === null ? null : points[active];
  const activeX = active === null ? 0 : x(active);
  const activeY = activePoint ? y(activePoint.value) : 0;

  return (
    <div
      ref={container}
      className="history-chart market-trend-chart"
      onPointerLeave={() => {
        setActive(null);
        setCursor(null);
      }}
    >
      <svg
        viewBox={`0 0 ${width} 232`}
        role="img"
        aria-labelledby={id}
        onPointerMove={(event) => {
          setCursor({
            container: event.currentTarget.closest('dialog'),
            x: event.clientX,
            y: event.clientY,
          });
          const rect = event.currentTarget.getBoundingClientRect();
          const target = ((event.clientX - rect.left) / rect.width) * width;
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
          {colorBySign && (
            <linearGradient
              id={`${id}-sign`}
              gradientUnits="userSpaceOnUse"
              x1="0"
              y1="30"
              x2="0"
              y2="190"
            >
              <stop offset={zeroOffset} stopColor="var(--price-up)" />
              <stop offset={zeroOffset} stopColor="var(--price-down)" />
            </linearGradient>
          )}
          <linearGradient id={`${id}-fill`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity=".3" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity=".02" />
          </linearGradient>
        </defs>
        {paths.map((path, index) => (
          <path
            key={index}
            d={`${path.line} L ${x(path.end)} ${areaBaseline} L ${x(path.start)} ${areaBaseline} Z`}
            fill={colorBySign ? seriesColor : `url(#${id}-fill)`}
            opacity={colorBySign ? 0.12 : undefined}
          />
        ))}
        {ticks.map((value, tick) => {
          return (
            <g key={tick}>
              <line x1={left} x2={right} y1={y(value)} y2={y(value)} className="chart-grid" />
              <text
                x={left - 10}
                y={y(value) + 4}
                textAnchor="end"
                style={
                  colorBySign
                    ? {
                        fill:
                          value > 0.05
                            ? 'var(--price-up)'
                            : value < -0.05
                              ? 'var(--price-down)'
                              : 'var(--faint)',
                      }
                    : undefined
                }
              >
                {tickLabel(value)}
              </text>
            </g>
          );
        })}
        {paths.map((path, index) => (
          <path
            key={index}
            d={path.line}
            className="chart-series-line"
            style={colorBySign ? { stroke: seriesColor } : undefined}
          />
        ))}
        {segments
          .filter((indices) => indices.length === 1)
          .map(([index]) => (
            <circle
              key={index}
              cx={x(index)}
              cy={y(points[index].value)}
              r="3"
              className="chart-dot"
              style={colorBySign ? { fill: seriesColor } : undefined}
            />
          ))}
        {activePoint && (
          <>
            <line x1={activeX} x2={activeX} y1="24" y2="190" className="chart-cursor" />
            <circle
              cx={activeX}
              cy={activeY}
              r="4.5"
              className="chart-dot"
              style={colorBySign ? { fill: seriesColor } : undefined}
            />
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
            onFocus={(event) => {
              if (!event.currentTarget.matches(':focus-visible')) return;
              setActive(index);
              const rect = event.currentTarget.getBoundingClientRect();
              setCursor({
                container: event.currentTarget.closest('dialog'),
                x: rect.left + rect.width / 2,
                y: rect.top,
              });
            }}
            onBlur={() => {
              setActive(null);
              setCursor(null);
            }}
          />
        ))}
        {(start === end ? [0.5] : width < 430 ? [0, 1] : [0, 0.5, 1]).map((tick) => (
          <text
            key={tick}
            x={left + (right - left) * tick}
            y="218"
            textAnchor={tick === 0 ? 'start' : tick === 1 ? 'end' : 'middle'}
          >
            {date(start + (end - start) * tick)}
          </text>
        ))}
      </svg>
      {activePoint && (
        <ChartTooltip cursor={cursor}>
          <span>{date(times[active!])}</span>
          <strong
            style={
              colorBySign
                ? {
                    color:
                      activePoint.value > 0.05
                        ? 'var(--price-up)'
                        : activePoint.value < -0.05
                          ? 'var(--price-down)'
                          : undefined,
                  }
                : undefined
            }
          >
            {valueLabel(activePoint.value)}
          </strong>
          <small>{activePoint.sampleSize.toLocaleString()} offers in this observation</small>
        </ChartTooltip>
      )}
    </div>
  );
}
