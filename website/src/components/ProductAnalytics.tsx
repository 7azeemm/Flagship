import { useEffect, useId, useState } from 'react';
import type { Product, VariantChart, VariantChartPoint } from '../types';
import { parseChart, request } from '../lib/api';
import { productHistoryChart } from '../lib/productHistory';
import { money } from '../lib/catalog';

const date = (timestamp: number) =>
  new Date(timestamp).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: '2-digit',
  });
export function HistoryChart({
  points,
  availability = false,
}: {
  points: VariantChartPoint[];
  availability?: boolean;
}) {
  const id = useId();
  const [selected, setSelected] = useState<number | null>(null);
  const [active, setActive] = useState<number | null>(null);
  const values = points.map((p) => (availability ? p.in_stock_count : p.lowest_price));
  const valid = values.filter((v): v is number => v !== null);
  if (!valid.length)
    return (
      <div className="history-empty">
        No {availability ? 'availability' : 'price'} observations in this period.
      </div>
    );
  const times = points.map((p) => Date.parse(p.timestamp));
  const start = times[0],
    end = times.at(-1)!;
  const low = availability
    ? 0
    : Math.max(
        0,
        Math.min(...valid) -
          Math.max(4, Math.max(...valid) * 0.03, (Math.max(...valid) - Math.min(...valid)) * 0.15),
      );
  const high = Math.max(
    availability ? 4 : low + 1,
    Math.max(...valid) +
      (availability
        ? 0
        : Math.max(4, Math.max(...valid) * 0.03, (Math.max(...valid) - Math.min(...valid)) * 0.15)),
  );
  const x = (i: number) => (end === start ? 352 : 64 + ((times[i] - start) / (end - start)) * 576);
  const y = (v: number) => 190 - ((v - low) / (high - low)) * 160;
  let path = '';
  values.forEach((v, i) => {
    if (v === null) return;
    path += i && values[i - 1] !== null ? ` H ${x(i)} V ${y(v)}` : ` M ${x(i)} ${y(v)}`;
  });
  const index = Math.min(active ?? selected ?? points.length - 1, points.length - 1);
  return (
    <div className="history-chart">
      <svg
        viewBox="0 0 670 232"
        role="img"
        aria-labelledby={id}
        onPointerLeave={() => setActive(null)}
        onPointerMove={(event) => {
          const rect = event.currentTarget.getBoundingClientRect();
          const target = ((event.clientX - rect.left) / rect.width) * 670;
          setActive(
            times.reduce(
              (best, _, i) => (Math.abs(x(i) - target) < Math.abs(x(best) - target) ? i : best),
              0,
            ),
          );
        }}
      >
        <title id={id}>
          {`${availability ? 'In-stock offers' : 'Recorded price in TND'} over time. Use the observation slider for exact values.`}
        </title>
        {[0, 1, 2, 3, 4].map((t) => {
          const v = low + ((high - low) * t) / 4;
          return (
            <g key={t}>
              <line x1="64" x2="640" y1={y(v)} y2={y(v)} className="chart-grid" />
              <text x="54" y={y(v) + 4} textAnchor="end">
                {availability ? Number(v.toFixed(1)) : money(v)}
              </text>
            </g>
          );
        })}
        <path
          d={path}
          className={availability ? 'availability-line' : 'price-line'}
          fill="none"
          strokeWidth="2.5"
          strokeLinejoin="round"
        />
        {values.map(
          (value, i) =>
            value !== null &&
            (i === 0 || values[i - 1] === null) && (
              <circle key={i} cx={x(i)} cy={y(value)} r="4" className="chart-dot" />
            ),
        )}
        <line x1={x(index)} x2={x(index)} y1="24" y2="190" className="chart-cursor" />
        {values[index] !== null && (
          <circle cx={x(index)} cy={y(values[index]!)} r="4" className="chart-dot" />
        )}
        {(start === end ? [0.5] : [0, 0.5, 1]).map((t) => (
          <text
            key={t}
            x={64 + 576 * t}
            y="218"
            textAnchor={t === 0 ? 'start' : t === 1 ? 'end' : 'middle'}
          >
            {end - start < 86400000 && start !== end
              ? new Date(start + (end - start) * t).toLocaleTimeString('en-GB', {
                  hour: '2-digit',
                  minute: '2-digit',
                })
              : date(start + (end - start) * t)}
          </text>
        ))}
      </svg>
      <div className="chart-readout">
        <span>{date(times[index])} · {new Date(times[index]).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })}</span>
        <strong>
          {values[index] === null
            ? 'Unavailable'
            : availability
              ? `${values[index]} of ${points[index].offer_count} offers in stock`
              : `${money(values[index]!)} TND`}
        </strong>
      </div>
      {points.length === 1 && (
        <p className="chart-single-observation">
          One recorded observation. More updates are needed to show a trend.
        </p>
      )}
      {points.length > 1 && (
        <label className="observation-control">
          <span>
            Drag to explore · {index + 1} of {points.length} observations
          </span>
          <input
            className="observation-slider"
            aria-label={availability ? 'Availability observation' : 'Price observation'}
            type="range"
            step="1"
            min="0"
            max={points.length - 1}
            value={index}
            aria-valuetext={`${date(times[index])}: ${values[index] ?? 'Unavailable'}${availability ? ' in stock' : ' TND'}`}
            onChange={(e) => {
              setActive(null);
              setSelected(Number(e.target.value));
            }}
          />
          {points.length === 2 && <span>Only two observations: the handle snaps between the two ends.</span>}
        </label>
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
    () => initialChart || productHistoryChart(product, '90d', 'auto'),
  );
  const [range, setRange] = useState(initialChart?.range || '90d');
  const [interval, setInterval] = useState('auto');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (
      initialChart &&
      range === (initialChart?.range || '90d') &&
      interval === 'auto' &&
      revision === 0
    ) {
      setChart(initialChart);
      setLoading(false);
      setError('');
      return;
    }
    if (!initialChart) {
      setChart(productHistoryChart(product, range, interval));
      setLoading(false);
      setError('');
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError('');
    void request(`${endpoint}?${new URLSearchParams({ range, interval })}`, controller.signal)
      .then((raw) => {
        if (controller.signal.aborted) return;
        const result = parseChart((raw as { chart?: unknown }).chart);
        if (!result) throw new Error('Price history is unavailable for this product.');
        setChart(result);
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'Chart unavailable.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [endpoint, range, interval, revision, initialChart, product]);
  return (
    <aside className="detail-analytics" aria-label="Product analytics">
      <div className="analytics-heading">
        <div>
          <span className="eyebrow">Market insights</span>
          <h2>Price & availability</h2>
          <p>
            {chart?.scope === 'product'
              ? 'Price and availability recorded for this retailer listing.'
              : 'Lowest in-stock price across matching listings; individual retailer prices may differ.'}
          </p>
        </div>
        <div className="chart-controls">
          <label>
            Period
            <select value={range} onChange={(e) => setRange(e.target.value)}>
              {[
                ['30d', '30 days'],
                ['90d', '90 days'],
                ['180d', '6 months'],
                ['1y', '1 year'],
                ['all', 'All time'],
              ].map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Interval
            <select value={interval} onChange={(e) => setInterval(e.target.value)}>
              {[
                ['auto', 'Automatic'],
                ['scan', 'Each change'],
                ['day', 'Daily'],
                ['week', 'Weekly'],
              ].map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
        </div>
      </div>
      <details className="chart-guide">
        <summary>How to read these charts</summary>
        <p>Period selects how far back to look. Interval controls the detail: Each change shows individual observations; Daily and Weekly keep the last observation in each day or week. Automatic chooses an interval for the selected period.</p>
        <p>Drag the bar below a chart, click its track, or use the arrow keys to select an observation and see its date and value. It moves in recorded steps. With two observations it snaps between the ends; with one, there is no slider. It does not zoom the chart.</p>
      </details>
      {loading && <p role="status">Updating charts…</p>}
      {error ? (
        <p role="alert">
          {error}{' '}
          <button className="text-button" onClick={() => setRevision((v) => v + 1)}>
            Retry
          </button>
        </p>
      ) : (
        <div aria-busy={loading} className={`analytics-body ${loading ? 'analytics-loading' : ''}`}>
          <div className="chart-stats">
            {[
              [
                chart?.scope === 'product' ? 'Current price' : 'Current lowest',
                chart?.summary.current_price,
              ],
              ['Period low', chart?.summary.period_low],
              ['All-time low', chart?.summary.all_time_low],
              ['Average observation', chart?.summary.average_price],
            ].map(([label, value]) => (
              <div key={label}>
                <span>{label}</span>
                <strong>
                  {typeof value === 'number' ? money(value) : '—'} <small>TND</small>
                </strong>
              </div>
            ))}
          </div>
          <section className="history-panel">
            <div className="history-heading">
              <h3>Price history</h3>
              <span>TND · {chart?.interval || interval}</span>
            </div>
            <HistoryChart key={`${range}-${interval}-price`} points={chart?.points || []} />
          </section>
          <section className="history-panel">
            <div className="history-heading">
              <h3>Availability history</h3>
              <span>In-stock offers</span>
            </div>
            <HistoryChart
              key={`${range}-${interval}-stock`}
              points={chart?.points || []}
              availability
            />
          </section>
        </div>
      )}
      <p className="detail-disclaimer">
        Daily and weekly charts show the last observation in each interval. Confirm prices and
        availability with the retailer.
      </p>
    </aside>
  );
}
