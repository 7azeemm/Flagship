import { useEffect, useState } from 'react';
import { Layers3, History } from 'lucide-react';
import type { VariantChart } from '../types';
import { parseChart, request } from '../lib/api';
import { fieldLabel, money } from '../lib/catalog';
import { RetailerHistoryChart } from './RetailerHistoryChart';

const ranges = [
  ['7d', '7D'],
  ['30d', '1M'],
  ['90d', '3M'],
  ['1y', '1Y'],
  ['all', 'All'],
] as const;

export function ProductAnalytics({
  initialChart,
  endpoint,
}: {
  initialChart: VariantChart;
  endpoint: string;
}) {
  const [chart, setChart] = useState(initialChart);
  const [range, setRange] = useState(initialChart.range || '90d');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setError('');
    if (range === initialChart.range && revision === 0) {
      setChart(initialChart);
      setLoading(false);
      return () => controller.abort();
    }
    setLoading(true);
    void request(
      `${endpoint}?${new URLSearchParams({ range, interval: 'auto' })}`,
      controller.signal,
    )
      .then((raw) => {
        if (controller.signal.aborted) return;
        const result = parseChart((raw as { chart?: unknown }).chart);
        if (!result) throw new Error('Price history is unavailable.');
        setChart(result);
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'Price history is unavailable.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [initialChart, endpoint, range, revision]);

  const current = chart.summary.current_price;
  const average = chart.summary.ninety_day_average;
  return (
    <aside
      className="detail-analytics simple-analytics price-observatory"
      aria-label="Price history"
    >
      <header className="price-history-header">
        <div>
          <span className="eyebrow">
            <History size={14} /> Price history
          </span>
          <h2>A little perspective on the price.</h2>
          <p>
            {chart.scope === 'variant'
              ? 'The best in-stock offer, followed over time.'
              : 'This listing’s recorded price, over time.'}
          </p>
        </div>
        <span className="history-currency">TND / DT</span>
      </header>
      {Object.keys(chart.combination_fields || {}).length > 0 && (
        <section className="history-configuration" aria-label="Compared configuration">
          <div className="configuration-label">
            <Layers3 size={16} />
            <strong>Compared configuration</strong>
            <span>Matching these specifications</span>
          </div>
          <dl>
            {Object.entries(chart.combination_fields || {}).map(([key, value]) => (
              <div key={key}>
                <dt>{fieldLabel(key)}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
        </section>
      )}
      <div className="history-period-bar">
        <span>Explore a period</span>
        <div className="range-tabs" aria-label="History period">
          {ranges.map(([value, label]) => (
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
      </div>
      <div className="simple-history-content" aria-busy={loading}>
        {loading ? (
          <p className="history-empty" role="status">
            Loading price history…
          </p>
        ) : error ? (
          <p role="alert">
            {error}{' '}
            <button className="text-button" onClick={() => setRevision((value) => value + 1)}>
              Retry
            </button>
          </p>
        ) : (
          <>
            <section className="price-insight" aria-label="Price summary">
              <div className="price-insight-main">
                <span className="insight-caption">
                  {chart.scope === 'variant' ? 'Current lowest price' : 'Current price'}
                </span>
                <div className="insight-price">
                  {current === null ? (
                    'Unavailable'
                  ) : (
                    <>
                      {money(current)}
                      <small>DT</small>
                    </>
                  )}
                </div>
              </div>
              <div className="price-insight-comparison">
                <span className="insight-caption">90-day average</span>
                <div className="insight-price">
                  {average === null ? (
                    'Unavailable'
                  ) : (
                    <>
                      {money(average)}
                      <small>DT</small>
                    </>
                  )}
                </div>
              </div>
            </section>
            <div className="analytics-charts">
              <section className="history-panel">
                <div className="history-heading">
                  <div>
                    <h3>Price</h3>
                    <p>{chart.scope === 'variant' ? 'Lowest in-stock price' : 'Listed price'}</p>
                  </div>
                  <span>DT</span>
                </div>
                <RetailerHistoryChart
                  key={endpoint + '-' + chart.range + '-price'}
                  points={chart.points}
                  scope={chart.scope}
                />
              </section>
              <section className="history-panel">
                <div className="history-heading">
                  <div>
                    <h3>Availability</h3>
                    <p>Listings in stock over time</p>
                  </div>
                  <span>Offers</span>
                </div>
                <RetailerHistoryChart
                  key={endpoint + '-' + chart.range + '-stock'}
                  points={chart.points}
                  scope={chart.scope}
                  availability
                />
              </section>
            </div>
          </>
        )}
      </div>
      <p className="detail-disclaimer">
        Recorded during catalog scans. Gaps indicate no available price. Confirm current price and
        stock with the retailer.
      </p>
    </aside>
  );
}
