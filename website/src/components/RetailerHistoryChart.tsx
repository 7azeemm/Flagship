import { useId, useState } from 'react';
import type { VariantChartPoint } from '../types';
import { money } from '../lib/catalog';
import { ChartTooltip, type ChartCursor } from './ChartTooltip';

export function RetailerHistoryChart({
  points,
  scope = 'variant',
  availability = false,
}: {
  points: VariantChartPoint[];
  scope?: 'variant' | 'product';
  availability?: boolean;
}) {
  const id = useId();
  const [active, setActive] = useState<number | null>(null);
  const [cursor, setCursor] = useState<ChartCursor | null>(null);
  const observations = points
    .filter((point) => Number.isFinite(Date.parse(point.timestamp)))
    .sort((a, b) => Date.parse(a.timestamp) - Date.parse(b.timestamp));
  const values = observations.map((point) => {
    if (availability) return point.in_stock_count;
    const value = point.lowest_price;
    return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null;
  });
  const prices = values.filter((value): value is number => value !== null);
  const times = observations.map((point) => Date.parse(point.timestamp));
  const start = times[0] || 0;
  const end = times.at(-1) || start;
  const low = prices.length ? Math.min(...prices) : 0;
  const high = prices.length ? Math.max(...prices) : 1;
  const padding = Math.max(availability ? 1 : 5, (high - low) * 0.15);
  const min = availability ? 0 : Math.max(0, low - padding);
  const max = availability ? Math.max(1, Math.ceil(high)) : high + padding;
  const x = (index: number) =>
    end === start ? 354 : 76 + ((times[index] - start) / (end - start)) * 556;
  const y = (value: number) => 190 - ((value - min) / (max - min)) * 160;
  const path = values
    .map((value, index) =>
      value === null
        ? ''
        : index === 0 || values[index - 1] === null
          ? `M ${x(index)} ${y(value)}`
          : `H ${x(index)} V ${y(value)}`,
    )
    .join(' ');
  const selected = Math.min(active ?? observations.length - 1, observations.length - 1);
  const observation = observations[selected];
  const date = (value: number) =>
    new Date(value).toLocaleDateString('en-GB', {
      day: 'numeric',
      month: 'short',
      year: '2-digit',
    });
  const label = availability
    ? 'Listings in stock'
    : scope === 'variant'
      ? 'Lowest in stock'
      : 'Listed price';

  return (
    <div className="simple-history">
      {prices.length ? (
        <svg
          viewBox="0 0 670 232"
          role="img"
          aria-labelledby={id}
          className="simple-history-svg"
          tabIndex={0}
          onFocus={(event) => {
            if (!event.currentTarget.matches(':focus-visible')) return;
            const rect = event.currentTarget.getBoundingClientRect();
            setActive(observations.length - 1);
            setCursor({
              container: event.currentTarget.closest('dialog'),
              x: rect.right - 30,
              y: rect.top + 30,
            });
          }}
          onKeyDown={(event) => {
            if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
            event.preventDefault();
            setActive(
              event.key === 'Home'
                ? 0
                : event.key === 'End'
                  ? observations.length - 1
                  : Math.max(
                      0,
                      Math.min(
                        observations.length - 1,
                        selected + (event.key === 'ArrowLeft' ? -1 : 1),
                      ),
                    ),
            );
            const rect = event.currentTarget.getBoundingClientRect();
            setCursor({
              container: event.currentTarget.closest('dialog'),
              x: rect.right - 30,
              y: rect.top + 30,
            });
          }}
          onBlur={() => {
            setActive(null);
            setCursor(null);
          }}
          onPointerLeave={() => {
            setActive(null);
            setCursor(null);
          }}
          onPointerMove={(event) => {
            setCursor({
              container: event.currentTarget.closest('dialog'),
              x: event.clientX,
              y: event.clientY,
            });
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
          <title
            id={id}
          >{`${label} over time${availability ? '' : ' in Tunisian dinars'}. Use left and right arrow keys to inspect observations.`}</title>
          {Array.from({ length: availability ? Math.min(3, max) + 1 : 4 }, (_, tick) => tick).map(
            (tick) => {
              const value = availability
                ? Math.round((max * tick) / Math.min(3, max))
                : min + ((max - min) * tick) / 3;
              return (
                <g key={tick}>
                  <line x1="76" x2="632" y1={y(value)} y2={y(value)} className="chart-grid" />
                  <text x="66" y={y(value) + 4} textAnchor="end">
                    {money(Math.round(value))}
                  </text>
                </g>
              );
            },
          )}
          <path
            d={path}
            fill="none"
            stroke="var(--accent)"
            strokeWidth="3"
            strokeLinejoin="round"
          />
          {values.map((value, index) =>
            value !== null &&
            (index === 0 ||
              values[index - 1] === null ||
              index === values.length - 1 ||
              values[index + 1] === null) ? (
              <circle key={index} cx={x(index)} cy={y(value)} r="3" fill="var(--accent)" />
            ) : null,
          )}
          {observation && active !== null && (
            <>
              <line x1={x(selected)} x2={x(selected)} y1="25" y2="195" className="chart-cursor" />
              {values[selected] !== null && (
                <circle cx={x(selected)} cy={y(values[selected]!)} r="5" fill="var(--accent)" />
              )}
            </>
          )}
          <text x="76" y="220">
            {date(start)}
          </text>
          {end !== start && (
            <text x="632" y="220" textAnchor="end">
              {date(end)}
            </text>
          )}
        </svg>
      ) : (
        <p className="history-empty">
          No recorded {availability ? 'availability' : 'prices'} in this period.
        </p>
      )}
      {observation && active !== null && (
        <ChartTooltip cursor={cursor}>
          <span>{new Date(times[selected]).toLocaleString('en-GB')}</span>
          <strong>
            {availability
              ? `${observation.in_stock_count} of ${observation.offer_count} listings in stock`
              : values[selected] === null
                ? 'Unavailable'
                : `${money(values[selected]!)} DT`}
          </strong>
          <small>
            {availability
              ? `${observation.retailer_count} retailers`
              : observation.lowest_price_retailer || label}
          </small>
        </ChartTooltip>
      )}
      {observations.length === 1 && (
        <p className="series-note">
          One observation recorded. More scans are needed to show a trend.
        </p>
      )}
    </div>
  );
}
