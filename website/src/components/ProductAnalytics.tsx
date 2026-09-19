import { useEffect, useId, useState } from 'react';
import type { Product, VariantChart, VariantChartPoint } from '../types';
import { parseChart, request } from '../lib/api';
import { buildProductHistoryChart } from '../lib/productHistory';
import { money } from '../lib/catalog';

const RANGES = [
  ['7d', '7D'],
  ['30d', '1M'],
  ['90d', '3M'],
  ['180d', '6M'],
  ['1y', '1Y'],
  ['all', 'All'],
] as const;

const date = (timestamp: number) =>
  new Date(timestamp).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: '2-digit',
  });

const dateTime = (timestamp: number) =>
  new Date(timestamp).toLocaleString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });

type PriceRating = {
  label: string;
  tone: 'exceptional' | 'great' | 'good' | 'typical' | 'high';
};

export function classifyPrice(current: number, average: number): PriceRating {
  const difference = ((current - average) / average) * 100;
  if (difference <= -15) return { label: 'Exceptional Price', tone: 'exceptional' };
  if (difference <= -8) return { label: 'Great Price', tone: 'great' };
  if (difference <= -3) return { label: 'Good Price', tone: 'good' };
  if (difference <= 8) return { label: 'Typical Price', tone: 'typical' };
  return { label: 'High Price', tone: 'high' };
}

function comparisonText(current: number, average: number) {
  const difference = ((current - average) / average) * 100;
  if (Math.abs(difference) < 0.05) return 'At the 90-day average';
  return `${difference < 0 ? '↓' : '↑'} ${Math.abs(difference).toFixed(1)}% ${
    difference < 0 ? 'below' : 'above'
  } 90-day average`;
}

export function HistoryChart({
  points,
  availability = false,
  scope = 'variant',
}: {
  points: VariantChartPoint[];
  availability?: boolean;
  scope?: 'product' | 'variant';
}) {
  const id = useId();
  const [active, setActive] = useState<number | null>(null);
  const values = points.map((point) => (availability ? point.in_stock_count : point.lowest_price));
  const valid = values.filter((value): value is number => value !== null);
  if (!valid.length)
    return (
      <div className="history-empty">
        No {availability ? 'availability' : 'price'} observations in this period.
      </div>
    );

  const times = points.map((point) => Date.parse(point.timestamp));
  const start = times[0];
  const end = times.at(-1)!;
  const lowest = Math.min(...valid);
  const highest = availability
    ? Math.max(1, ...points.map((point) => point.offer_count))
    : Math.max(...valid);
  const padding = availability ? 0 : Math.max(4, highest * 0.03, (highest - lowest) * 0.15);
  const low = availability ? 0 : Math.max(0, lowest - padding);
  const high = Math.max(availability ? 1 : low + 1, highest + padding);
  const ticks =
    availability && high <= 4
      ? Array.from({ length: high + 1 }, (_, value) => value)
      : [0, 1, 2, 3, 4].map((tick) => low + ((high - low) * tick) / 4);
  const x = (index: number) =>
    end === start ? 352 : 64 + ((times[index] - start) / (end - start)) * 576;
  const y = (value: number) => 190 - ((value - low) / (high - low)) * 160;
  const segments: Array<Array<{ index: number; value: number }>> = [];
  values.forEach((value, index) => {
    if (value === null) return;
    if (index === 0 || values[index - 1] === null) segments.push([]);
    segments.at(-1)!.push({ index, value });
  });
  const segmentPath = (segment: Array<{ index: number; value: number }>) => {
    const first = segment[0];
    if (segment.length === 1) {
      const center = x(first.index);
      return `M ${Math.max(64, center - 8)} ${y(first.value)} L ${Math.min(640, center + 8)} ${y(first.value)}`;
    }
    return segment.slice(1).reduce(
      (path, point, index) => {
        const previous = segment[index];
        const middle = (x(previous.index) + x(point.index)) / 2;
        return `${path} C ${middle} ${y(previous.value)}, ${middle} ${y(point.value)}, ${x(point.index)} ${y(point.value)}`;
      },
      `M ${x(first.index)} ${y(first.value)}`,
    );
  };
  const path = segments.map(segmentPath).join(' ');
  const area = segments
    .map((segment) => {
      const first = segment[0];
      const last = segment.at(-1)!;
      const firstX = segment.length === 1 ? Math.max(64, x(first.index) - 8) : x(first.index);
      const lastX = segment.length === 1 ? Math.min(640, x(last.index) + 8) : x(last.index);
      return `${segmentPath(segment)} L ${lastX} 190 L ${firstX} 190 Z`;
    })
    .join(' ');

  const activePoint = active === null ? null : points[active];
  const activeValue = active === null ? null : values[active];
  const activeX = active === null ? 0 : x(active);
  const activeY = activeValue === null ? 190 : y(activeValue);
  const accessibleLabel = (point: VariantChartPoint, value: number | null) =>
    availability
      ? `${dateTime(Date.parse(point.timestamp))}: ${point.in_stock_count} of ${point.offer_count} offers in stock`
      : `${dateTime(Date.parse(point.timestamp))}: ${value === null ? 'unavailable' : `${money(value)} TND`}${
          point.lowest_price_retailer ? ` at ${point.lowest_price_retailer}` : ''
        }`;

  return (
    <div className="history-chart" onPointerLeave={() => setActive(null)}>
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
        <title id={id}>
          {availability
            ? 'Number of in-stock offers over time'
            : scope === 'product'
              ? 'Listed price over time, in TND'
              : 'Lowest in-stock price over time, in TND'}
        </title>
        <defs>
          <linearGradient id={`${id}-fill`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor={availability ? '#10b981' : '#818cf8'} stopOpacity=".3" />
            <stop
              offset="100%"
              stopColor={availability ? '#10b981' : '#818cf8'}
              stopOpacity=".02"
            />
          </linearGradient>
        </defs>
        <path d={area} fill={`url(#${id}-fill)`} />
        {ticks.map((value) => {
          return (
            <g key={value}>
              <line x1="64" x2="640" y1={y(value)} y2={y(value)} className="chart-grid" />
              <text x="54" y={y(value) + 4} textAnchor="end">
                {availability ? Math.round(value) : money(value)}
              </text>
            </g>
          );
        })}
        <path
          d={path}
          className="chart-series-line"
          stroke={availability ? '#059669' : '#6366f1'}
          fill="none"
          strokeWidth="3.25"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        {active !== null && (
          <>
            <line x1={activeX} x2={activeX} y1="24" y2="190" className="chart-cursor" />
            {activeValue !== null && (
              <circle
                cx={activeX}
                cy={activeY}
                r="4.5"
                className="chart-dot"
                fill={availability ? '#059669' : '#6366f1'}
              />
            )}
          </>
        )}
        {points.map((point, index) => (
          <circle
            key={`${point.timestamp}-${index}`}
            cx={x(index)}
            cy={values[index] === null ? 190 : y(values[index]!)}
            r="10"
            className="chart-hit-target"
            tabIndex={0}
            aria-label={accessibleLabel(point, values[index])}
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
          <span>{dateTime(times[active!])}</span>
          <strong>
            {availability
              ? `${activePoint.in_stock_count} of ${activePoint.offer_count} in stock`
              : activeValue === null
                ? 'Unavailable'
                : `${money(activeValue)} TND`}
          </strong>
          <small>
            {availability
              ? `${activePoint.retailer_count} retailer${activePoint.retailer_count === 1 ? '' : 's'}`
              : activePoint.lowest_price_retailer || 'Retailer unavailable'}
          </small>
        </div>
      )}
      {points.length <= 2 && (
        <p className="chart-single-observation">
          One recorded observation. More updates are needed to show a trend.
        </p>
      )}
    </div>
  );
}

export function ProductAnalytics({
  initialChart,
  endpoint,
  product,
}: {
  initialChart: VariantChart | null;
  endpoint: string;
  product: Product;
}) {
  const [chart, setChart] = useState(
    () => initialChart || buildProductHistoryChart(product, '90d', 'auto'),
  );
  const [range, setRange] = useState(initialChart?.range || '90d');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    if (initialChart && range === (initialChart.range || '90d') && revision === 0) {
      setChart(initialChart);
      setLoading(false);
      setError('');
      return;
    }
    if (!initialChart) {
      setChart(buildProductHistoryChart(product, range, 'auto'));
      setLoading(false);
      setError('');
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError('');
    void request(
      `${endpoint}?${new URLSearchParams({ range, interval: 'auto' })}`,
      controller.signal,
    )
      .then((raw) => {
        if (controller.signal.aborted) return;
        const result = parseChart((raw as { chart?: unknown }).chart);
        if (!result) throw new Error('Price history is unavailable for this product.');
        setChart(result);
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'Charts unavailable.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [endpoint, range, revision, initialChart, product]);

  const current = chart?.summary.current_price;
  const average90 = chart?.summary.ninety_day_average;
  const rating =
    chart.points.length > 2 &&
    typeof current === 'number' &&
    typeof average90 === 'number' &&
    average90 > 0
      ? classifyPrice(current, average90)
      : null;

  return (
    <aside className="detail-analytics" aria-label="Product analytics">
      <div className="analytics-heading">
        <div>
          <span className="eyebrow">Market insights</span>
          <h2>Price & availability</h2>
          <p>
            {chart?.scope === 'product'
              ? 'Price and availability recorded for this retailer listing.'
              : 'Lowest in-stock price across matching retailer listings.'}
          </p>
        </div>
      </div>

      {rating && typeof current === 'number' && typeof average90 === 'number' && (
        <div className="price-classification" data-tone={rating.tone}>
          <span className="price-rating-dot" aria-hidden="true" />
          <div>
            <strong>{rating.label}</strong>
            <span>{comparisonText(current, average90)}</span>
          </div>
          <small>Compared with {money(average90)} TND average over 90 days</small>
        </div>
      )}

      <div className="chart-stats">
        {[
          [chart?.scope === 'product' ? 'Current price' : 'Current lowest', current],
          ['Lowest', chart?.summary.period_low],
          ['Average', chart?.summary.average_price],
          ['Highest', chart?.summary.period_high],
        ].map(([label, value]) => (
          <div key={label}>
            <span>{label}</span>
            <strong>
              {typeof value === 'number' ? money(value) : '—'} <small>TND</small>
            </strong>
          </div>
        ))}
      </div>

      <div className="chart-toolbar">
        <div className="range-tabs" aria-label="Chart range">
          {RANGES.map(([value, label]) => (
            <button
              key={value}
              type="button"
              aria-pressed={range === value}
              onClick={() => setRange(value)}
            >
              {label}
            </button>
          ))}
        </div>
        <span>Interval: automatic</span>
      </div>

      {loading && (
        <p role="status" className="chart-status">
          Updating charts…
        </p>
      )}
      {error ? (
        <p role="alert">
          {error}{' '}
          <button className="text-button" onClick={() => setRevision((value) => value + 1)}>
            Retry
          </button>
        </p>
      ) : (
        <div
          aria-busy={loading}
          className={`analytics-charts ${loading ? 'analytics-loading' : ''}`}
        >
          <section className="history-panel">
            <div className="history-heading">
              <div>
                <h3>{chart?.scope === 'product' ? 'Listed price' : 'Lowest price'}</h3>
                <p>
                  {chart?.scope === 'product'
                    ? 'Price recorded for this listing'
                    : 'Cheapest in-stock offer'}
                </p>
              </div>
              <span>TND</span>
            </div>
            <HistoryChart
              key={`${range}-price`}
              points={chart?.points || []}
              scope={chart?.scope}
            />
          </section>
          <section className="history-panel">
            <div className="history-heading">
              <div>
                <h3>Availability</h3>
                <p>Offers currently in stock</p>
              </div>
              <span>In-stock offers</span>
            </div>
            <HistoryChart key={`${range}-stock`} points={chart?.points || []} availability />
          </section>
        </div>
      )}
      <p className="detail-disclaimer">
        Hover or focus a chart point for exact price, retailer, and availability details. Confirm
        prices and stock with the retailer.
      </p>
    </aside>
  );
}
