import { useId, useState } from 'react';
import type { RetailerSeries } from '../lib/retailerHistory';
import { money } from '../lib/catalog';

const color = (index: number) =>
  ['#818cf8', '#10b981', '#f59e0b', '#f472b6', '#38bdf8', '#a78bfa', '#fb7185', '#2dd4bf'][index] ??
  `hsl(${(index * 137.508) % 360} 70% 60%)`;
const date = (timestamp: string) =>
  new Date(timestamp).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' });

export function RetailerPriceChart({ series }: { series: RetailerSeries[] }) {
  const id = useId();
  const [hidden, setHidden] = useState<string[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const times = [
    ...new Set(series.flatMap((line) => line.points.map((point) => point.timestamp))),
  ].sort();
  const prices = series.flatMap((line) =>
    line.points.flatMap((point) => (point.lowest_price === null ? [] : [point.lowest_price])),
  );
  if (!prices.length || !times.length)
    return <p className="history-empty">No in-stock retailer prices recorded for this period.</p>;
  const start = Date.parse(times[0]),
    end = Date.parse(times.at(-1)!);
  const low = Math.max(0, Math.min(...prices) * 0.97),
    high = Math.max(low + 1, Math.max(...prices) * 1.03);
  const x = (timestamp: string) =>
    start === end ? 352 : 64 + ((Date.parse(timestamp) - start) / (end - start)) * 576;
  const y = (price: number) => 190 - ((price - low) / (high - low)) * 160;
  const index = Math.min(selected ?? times.length - 1, times.length - 1);
  return (
    <div className="history-chart retailer-chart">
      <svg viewBox="0 0 670 232" role="img" aria-labelledby={id}>
        <title id={id}>Lowest in-stock price per retailer over time, in TND</title>
        {[0, 1, 2, 3, 4].map((tick) => {
          const value = low + ((high - low) * tick) / 4;
          return (
            <g key={tick}>
              <line x1="64" x2="640" y1={y(value)} y2={y(value)} className="chart-grid" />
              <text x="54" y={y(value) + 4} textAnchor="end">
                {money(value)}
              </text>
            </g>
          );
        })}
        {series.map((line, lineIndex) => {
          if (hidden.includes(line.site)) return null;
          let path = '';
          line.points.forEach((point, index) => {
            if (point.lowest_price !== null)
              path +=
                index && line.points[index - 1].lowest_price !== null
                  ? ` H ${x(point.timestamp)} V ${y(point.lowest_price)}`
                  : ` M ${x(point.timestamp)} ${y(point.lowest_price)}`;
          });
          return (
            <g key={line.site}>
              <path
                d={path}
                fill="none"
                stroke={color(lineIndex)}
                strokeWidth="2.5"
                strokeLinejoin="round"
              />
              {line.points
                .filter(
                  (point, index) =>
                    point.lowest_price !== null &&
                    (index === 0 ||
                      line.points[index - 1].lowest_price === null ||
                      index === line.points.length - 1),
                )
                .map((point) => (
                  <circle
                    key={point.timestamp}
                    cx={x(point.timestamp)}
                    cy={y(point.lowest_price!)}
                    r="3.5"
                    fill={color(lineIndex)}
                  />
                ))}
            </g>
          );
        })}
        <line x1={x(times[index])} x2={x(times[index])} y1="24" y2="190" className="chart-cursor" />
        <text x="64" y="218">
          {date(times[0])}
        </text>
        <text x="640" y="218" textAnchor="end">
          {date(times.at(-1)!)}
        </text>
      </svg>
      <div className="chart-readout">
        <span>{date(times[index])}</span>
        <strong>{index === times.length - 1 ? 'Latest prices' : 'Recorded prices'} · TND</strong>
      </div>
      {times.length > 1 && (
        <input
          className="observation-slider"
          type="range"
          aria-label="Retailer price observation"
          min="0"
          max={times.length - 1}
          value={index}
          aria-valuetext={times[index]}
          onChange={(event) => setSelected(Number(event.target.value))}
        />
      )}
      <div className="retailer-legend">
        {series.map((line, index) => {
          const point = line.points
            .filter(
              (point) =>
                point.timestamp <= times[Math.min(selected ?? times.length - 1, times.length - 1)],
            )
            .at(-1);
          return (
            <button
              key={line.site}
              aria-pressed={!hidden.includes(line.site)}
              onClick={() =>
                setHidden(
                  hidden.includes(line.site)
                    ? hidden.filter((site) => site !== line.site)
                    : [...hidden, line.site],
                )
              }
            >
              <i style={{ background: color(index) }} />
              <span>{line.site}</span>
              <strong>
                {point?.lowest_price == null ? 'Unavailable' : money(point.lowest_price)}
              </strong>
            </button>
          );
        })}
      </div>
      <p className="chart-single-observation">
        Each line follows one retailer’s cheapest in-stock offer. Select a retailer to hide or show
        its line.
      </p>
    </div>
  );
}

export function PricePositionChart({ series }: { series: RetailerSeries[] }) {
  const id = useId();
  const available = series.filter((line) => line.low !== null && line.high !== null);
  if (!available.length)
    return <p className="history-empty">No in-stock prices to compare in this period.</p>;
  const low = Math.min(...available.map((line) => line.low!));
  const high = Math.max(...available.map((line) => line.high!));
  const x = (price: number) => (high === low ? 340 : 150 + ((price - low) / (high - low)) * 390);
  return (
    <div className="price-position-chart">
      <svg viewBox={`0 0 670 ${available.length * 62 + 36}`} role="img" aria-labelledby={id}>
        <title id={id}>
          Current retailer prices within their recorded period low and high, in TND
        </title>
        <defs>
          <linearGradient id={`${id}-range`}>
            <stop stopColor="#10b981" />
            <stop offset="1" stopColor="#818cf8" />
          </linearGradient>
        </defs>
        {available.map((line, index) => {
          const y = 25 + index * 62;
          return (
            <g key={line.site}>
              <text x="0" y={y + 4}>
                {line.site.length > 18 ? line.site.slice(0, 17) + '…' : line.site}
              </text>
              <line x1="150" x2="540" y1={y} y2={y} className="chart-grid" />
              <line
                x1={x(line.low!)}
                x2={x(line.high!)}
                y1={y}
                y2={y}
                stroke={`url(#${id}-range)`}
                strokeWidth="8"
                strokeLinecap="round"
              />
              {line.current !== null && (
                <circle
                  cx={x(line.current)}
                  cy={y}
                  r="6"
                  fill={color(series.indexOf(line))}
                  stroke="var(--surface)"
                  strokeWidth="2"
                />
              )}
              <text x="660" y={y + 4} textAnchor="end">
                {line.current === null ? 'Unavailable' : money(line.current)}
              </text>
              <text x="150" y={y + 23}>
                Low {money(line.low!)}
              </text>
              <text x="540" y={y + 23} textAnchor="end">
                High {money(line.high!)}
              </text>
            </g>
          );
        })}
      </svg>
      <p className="chart-single-observation">
        The dot is today’s lowest in-stock price. The colored range spans that retailer’s low and
        high for this period. A dot near the left is closer to its best recorded price.
      </p>
    </div>
  );
}
