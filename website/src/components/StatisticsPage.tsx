import { useEffect, useState } from 'react';
import {
  ArrowRight,
  BarChart3,
  CalendarDays,
  Info,
  RefreshCw,
  Store,
} from 'lucide-react';
import type { ServerInfo, StatisticChart, StatisticsData } from '../types';
import { parseStatistics, request } from '../lib/api';
import { money, sectionLabel } from '../lib/catalog';
import { MarketTrendChart } from './MarketTrendChart';
import '../statistics.css';

const ranges = [
  ['7d', '7 days'],
  ['30d', '30 days'],
  ['90d', '90 days'],
  ['180d', '6 months'],
  ['1y', '1 year'],
  ['all', 'All time'],
];
const statisticsCache = new Map<string, { data: StatisticsData; updated: number }>();

type RetailerMeasure = 'lowest' | 'offers';

function RetailerComparison({
  overviews,
  sections,
}: {
  overviews: StatisticsData['retailer_overviews'];
  sections: string[];
}) {
  const [measure, setMeasure] = useState<RetailerMeasure>('lowest');
  const [section, setSection] = useState('All');
  const [expanded, setExpanded] = useState(false);
  const overview = overviews[section] || overviews.All;
  if (!overview) return null;
  const hasPriceComparisons = overview.retailers.some((item) => item.price_comparisons > 0);
  const activeMeasure = measure === 'lowest' && !hasPriceComparisons ? 'offers' : measure;
  const choices: { id: RetailerMeasure; label: string }[] = [
    { id: 'lowest', label: 'Lowest-price wins' },
    { id: 'offers', label: 'Catalog size' },
  ];
  const ranked = overview.retailers
    .map((item) => ({
      ...item,
      value:
        activeMeasure === 'lowest'
          ? item.price_comparisons > 0
            ? (item.lowest_price_wins / item.price_comparisons) * 100
            : null
          : item.offers,
    }))
    .sort((a, b) => (b.value ?? -1) - (a.value ?? -1) || b.offers - a.offers || a.name.localeCompare(b.name));
  const entries = expanded ? ranked : ranked.slice(0, 8);
  const max = activeMeasure === 'offers' ? Math.max(1, ...ranked.map((item) => item.value ?? 0)) : 100;
  const scope = section === 'All' ? 'all categories' : sectionLabel(section).toLowerCase();
  const explanation =
    activeMeasure === 'lowest'
      ? `Current in-stock prices within ${money(overview.comparable_configurations)} shared configurations in ${scope}. Each retailer's percentage is its lowest-price wins divided by the configurations where it has a matching offer. Ties count; check the comparison count for small samples.`
      : `Number of tracked offers in ${scope} at the latest catalog snapshot.${hasPriceComparisons ? '' : ' No shared products are available for a price comparison yet.'}`;
  return (
    <div className="retailer-comparison">
      <div className="retailer-comparison-top">
        <div>
          <span className="home-kicker">Retailer comparison</span>
          <h3>Where retailers stand</h3>
          <p>{explanation}</p>
        </div>
        <div className="retailer-comparison-tools">
          <label>
            Category
            <select
              value={section}
              onChange={(event) => {
                setSection(event.target.value);
                if (!overviews[event.target.value]?.retailers.some((item) => item.price_comparisons > 0)) setMeasure('offers');
                setExpanded(false);
              }}
            >
              <option value="All">All categories</option>
              {sections
                .filter((item) => overviews[item])
                .map((item) => (
                  <option key={item} value={item}>
                    {sectionLabel(item)}
                  </option>
                ))}
            </select>
          </label>
          <div className="retailer-measures" role="group" aria-label="Retailer measure">
            {choices.map((choice) => (
              <button
                key={choice.id}
                type="button"
                className={activeMeasure === choice.id ? 'active' : ''}
                aria-pressed={activeMeasure === choice.id}
                disabled={choice.id === 'lowest' && !hasPriceComparisons}
                title={choice.id === 'lowest' && !hasPriceComparisons ? 'No shared products are available for comparison' : undefined}
                onClick={() => {
                  setMeasure(choice.id);
                  setExpanded(false);
                }}
              >
                {choice.label}
              </button>
            ))}
          </div>
        </div>
      </div>
      {entries.length > 0 ? (
        <div
          className="retailer-bars"
          aria-label={`Retailers ranked by ${choices.find((choice) => choice.id === activeMeasure)?.label.toLowerCase()}`}
        >
          {entries.map((item, index) => (
            <div className="retailer-bar-row" key={item.name}>
              <span className="retailer-bar-rank">{String(index + 1).padStart(2, '0')}</span>
              <strong title={item.name}>{item.name}</strong>
              <span className="retailer-bar-track">
                <span style={{ width: `${((item.value ?? 0) / max) * 100}%` }} />
              </span>
              <span className="retailer-bar-value">
                {activeMeasure === 'offers' ? money(item.offers) : item.value === null ? '—' : `${item.value.toFixed(0)}%`}
                <small>
                  {activeMeasure === 'lowest'
                    ? item.value === null
                      ? 'No shared matches'
                      : item.price_comparisons < 5
                        ? `${item.lowest_price_wins}/${item.price_comparisons} · small sample`
                        : `${item.lowest_price_wins}/${item.price_comparisons} comparisons`
                    : 'tracked offers'}
                </small>
              </span>
            </div>
          ))}
          <div className="retailer-bar-axis" aria-hidden="true">
            <div>
              <span>0</span>
              <span>{activeMeasure === 'offers' ? `${money(Math.round(max / 2))}` : '50%'}</span>
              <span>{activeMeasure === 'offers' ? `${money(max)} offers` : '100%'}</span>
            </div>
          </div>
        </div>
      ) : (
        <div className="statistics-empty retailer-empty">
          <Store size={26} />
          <strong>No retailer listings in this category</strong>
          <span>Listings will appear here after a catalog scan.</span>
        </div>
      )}
      <div className="retailer-comparison-foot">
        <span>
          {money(ranked.length)} retailers in this view
          {!expanded && ranked.length > 8 ? ' · top eight shown' : ''}
        </span>
        {ranked.length > 8 && (
          <button type="button" onClick={() => setExpanded((value) => !value)}>
            {expanded ? 'Show top eight' : `Show all ${money(ranked.length)}`}
          </button>
        )}
      </div>
    </div>
  );
}

function valueLabel(chart: StatisticChart, value: number) {
  if (chart.metric === 'availability') return `${value.toFixed(1)}%`;
  if (chart.metric === 'index') return `${value > 100 ? '+' : ''}${(value - 100).toFixed(1)}%`;
  return `${money(Math.round(value))} DT`;
}

function axisLabel(chart: StatisticChart, value: number) {
  if (chart.metric === 'availability') return `${Math.round(value)}%`;
  if (chart.metric === 'index')
    return `${Math.abs(value - 100) < 0.05 ? '0.0' : (value - 100).toFixed(1)}%`;
  return value >= 1000 ? `${(value / 1000).toFixed(1)}k DT` : `${Math.round(value)} DT`;
}

function StatisticVisual({ chart }: { chart: StatisticChart }) {
  const points = chart.points;
  const last = points.at(-1);
  const first = points[0];
  const title = chart.title;
  return (
    <article className="statistics-card">
      <div className="statistics-card-heading">
        <div>
          <span>
            {sectionLabel(chart.section)} · {chart.range.toUpperCase()}
          </span>
          <h3>{title}</h3>
          <p>{chart.description}</p>
        </div>
        {last && <strong>{valueLabel(chart, last.value)}</strong>}
      </div>
      {points.length > 1 ? (
        <MarketTrendChart
          title={`${title} over time: ${valueLabel(chart, first.value)} to ${valueLabel(chart, last!.value)}`}
          points={points.map((point) => ({
            timestamp: point.timestamp,
            value: point.value,
            sampleSize: point.sample_size,
          }))}
          valueLabel={(value) => valueLabel(chart, value)}
          tickLabel={(value) => axisLabel(chart, value)}
          baseline={chart.metric === 'index' ? 100 : undefined}
          minimumPadding={chart.metric === 'index' ? 0.5 : chart.metric === 'availability' ? 2 : 4}
        />
      ) : (
        <div className="statistics-empty">
          <BarChart3 size={27} />
          <strong>Not enough history yet</strong>
          <span>
            {chart.metric === 'index'
              ? 'At least five comparable offers across two observations are needed.'
              : 'This chart needs at least two supported observations.'}
          </span>
        </div>
      )}
      <div className="statistics-card-meta">
        <span>
          {money(chart.matching_offers)} matching offers · {chart.retailer_count} retailers
        </span>
        <span>
          {chart.metric === 'index'
            ? `${money(chart.comparable_offers)} comparable in latest interval`
            : last
              ? `${money(last.sample_size)} in latest point`
              : 'No sample yet'}
        </span>
      </div>
      <details>
        <summary>
          <Info size={14} /> How to read this chart
        </summary>
        <p>{chart.note}</p>
      </details>
    </article>
  );
}

export function StatisticsPage({
  categories,
  onNavigate,
}: {
  categories: ServerInfo['categories'];
  onNavigate: (path: string) => void;
}) {
  const [section, setSection] = useState('Memory');
  const [range, setRange] = useState('30d');
  const [retailer, setRetailer] = useState('');
  const [filters, setFilters] = useState<Record<string, string>>({});
  const [data, setData] = useState<StatisticsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const [selectedConfiguration, setSelectedConfiguration] = useState<string | null>(null);

  useEffect(() => {
    document.title = 'Statistics — Mythos';
  }, []);
  useEffect(() => {
    const controller = new AbortController();
    const params = new URLSearchParams({
      section,
      range,
      filters: JSON.stringify(filters),
    });
    if (retailer) params.set('retailer', retailer);
    const path = `/stats?${params}`;
    const cached = statisticsCache.get(path);
    if (cached && Date.now() - cached.updated < 30_000) {
      setData(cached.data);
      setLoading(false);
      setError('');
      return () => controller.abort();
    }
    setLoading(true);
    setError('');
    setSelectedConfiguration(null);
    request(path, controller.signal)
      .then(parseStatistics)
      .then((result) => {
        if (!controller.signal.aborted) {
          statisticsCache.set(path, { data: result, updated: Date.now() });
          if (statisticsCache.size > 32)
            statisticsCache.delete(statisticsCache.keys().next().value!);
          setData(result);
        }
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'Statistics are unavailable.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [section, range, retailer, filters, revision]);

  const sections = [
    ...new Set(
      Object.values(categories)
        .flat()
        .map((item) => item.id),
    ),
  ];
  if (!sections.includes('Memory')) sections.unshift('Memory');
  const activeRefinements = Object.keys(filters).length + (retailer ? 1 : 0);
  const explorer = data?.explorer_charts[0];
  const visibleFacets = explorer?.section === section ? data?.facets || [] : [];
  const visibleRetailers = explorer?.section === section ? data?.retailers || [] : [];
  const configuration =
    data?.configurations.find((item) => item.id === selectedConfiguration) ||
    data?.configurations[0];
  const configurationChart = (kind: 'lowest_price' | 'availability'): StatisticChart | null =>
    data && configuration
      ? {
          id: `${configuration.id}-${kind}`,
          title: kind === 'lowest_price' ? 'Lowest available price' : 'In-stock offers',
          description: configuration.title,
          section: explorer!.section,
          filters: explorer!.filters,
          metric: kind,
          range: explorer!.range,
          matching_offers: configuration.offers,
          retailer_count: configuration.retailer_count,
          comparable_offers: 0,
          points:
            kind === 'lowest_price'
              ? configuration.price_points
              : configuration.availability_points,
          note:
            kind === 'lowest_price'
              ? 'Lowest recorded in-stock price among offers in this configuration. Missing stock periods are not connected to a known price.'
              : 'Percentage of offers in this configuration recorded as in stock at each observation.',
        }
      : null;
  return (
    <main className="statistics-main" id="statistics-content" tabIndex={-1}>
      <div className="statistics-hero">
        <span className="home-kicker">The numbers behind the market</span>
        <h1>The tech market, in focus.</h1>
        <p>
          See how retailers compare, then explore recorded prices and availability for the products
          you care about.
        </p>
        {data?.last_scan_at && (
          <div className="statistics-freshness">
            <CalendarDays size={16} />
            Last scan {new Date(data.last_scan_at).toLocaleString()}
          </div>
        )}
      </div>
      <section className="statistics-section" aria-labelledby="retailer-title">
        <div className="statistics-section-heading">
          <div>
            <span className="home-kicker">The retailer picture</span>
            <h2 id="retailer-title">More than price alone</h2>
          </div>
          <p>Compare current listings and lowest-price wins.</p>
        </div>
        {data ? (
          <RetailerComparison overviews={data.retailer_overviews} sections={sections} />
        ) : loading ? (
          <div className="statistics-loading">Loading retailer comparison…</div>
        ) : null}
      </section>
      <section className="statistics-section" aria-labelledby="explorer-title">
        <div className="statistics-section-heading">
          <div>
            <span className="home-kicker">Make it specific</span>
            <h2 id="explorer-title">Explore the market</h2>
          </div>
          <p>One set of filters updates every chart and the matching configurations below.</p>
        </div>
        <div className="statistics-workbench">
          <div className="statistics-workbench-head">
            <div>
              <span className="home-kicker">Chart setup</span>
              <h3>Choose what to explore</h3>
            </div>
            <div className="statistics-primary-filters">
              <label className="statistics-category-select">
                Category
                <select
                  value={section}
                  onChange={(event) => {
                    setSection(event.target.value);
                    setFilters({});
                    setRetailer('');
                  }}
                >
                  {sections.map((item) => (
                    <option key={item} value={item}>
                      {sectionLabel(item)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="statistics-category-select">
                Period
                <select value={range} onChange={(event) => setRange(event.target.value)}>
                  {ranges.map(([value, label]) => (
                    <option key={value} value={value}>{label}</option>
                  ))}
                </select>
              </label>
            </div>
          </div>
          <div className="statistics-refine">
            <div className="statistics-refine-heading">
              <strong>Retailer & specifications</strong>
              {activeRefinements > 0 && <span>{activeRefinements} active</span>}
            </div>
              <div className="statistics-refine-body" id="statistics-refinements">
                <label>
                  Retailer
                  <select value={retailer} onChange={(event) => setRetailer(event.target.value)}>
                    <option value="">All retailers</option>
                    {visibleRetailers.map((item) => (
                      <option key={item} value={item}>
                        {item}
                      </option>
                    ))}
                  </select>
                </label>
                {visibleFacets.map((facet) => (
                  <label key={facet.key}>
                    {facet.label}
                    <select
                      value={filters[facet.key] || ''}
                      onChange={(event) =>
                        setFilters((current) => {
                          const next = { ...current };
                          if (event.target.value) next[facet.key] = event.target.value;
                          else delete next[facet.key];
                          return next;
                        })
                      }
                    >
                      <option value="">Any</option>
                      {facet.options.map((option) => (
                        <option key={option.value} value={option.value} disabled={option.count === 0 && filters[facet.key] !== option.value}>
                          {option.value} ({option.count})
                        </option>
                      ))}
                    </select>
                  </label>
                ))}
                {activeRefinements > 0 && (
                  <button
                    type="button"
                    className="statistics-clear"
                    onClick={() => {
                      setRetailer('');
                      setFilters({});
                    }}
                  >
                    Clear refinements
                  </button>
                )}
              </div>
          </div>
        </div>
        {loading && data && (
          <p className="statistics-updating" role="status">
            Updating this view…
          </p>
        )}
        {data && <div className="statistics-chart-grid">{data.explorer_charts.map((chart) => <StatisticVisual key={chart.id} chart={chart} />)}</div>}
        {data && (
          <div className="statistics-configurations">
            <div className="statistics-config-heading">
              <h3>Matching configurations</h3>
              <p>
                Showing up to 12 of {money(data.configuration_count)} current configurations. Narrow
                the filters to find a specific one.
              </p>
            </div>
            {data.configurations.length ? (
              <>
                <div
                  className="statistics-config-list"
                  role="group"
                  aria-label="Matching configurations"
                >
                  {data.configurations.map((item) => (
                    <button
                      key={item.id}
                      className={configuration?.id === item.id ? 'active' : ''}
                      aria-pressed={configuration?.id === item.id}
                      onClick={() => setSelectedConfiguration(item.id)}
                    >
                      <strong>{item.title}</strong>
                      <span>
                        {item.offers} {item.offers === 1 ? 'offer' : 'offers'} ·{' '}
                        {item.retailer_count} {item.retailer_count === 1 ? 'retailer' : 'retailers'}
                      </span>
                    </button>
                  ))}
                </div>
                {configuration && (
                  <div className="statistics-config-detail">
                    <div className="statistics-config-title">
                      <h4>{configuration.title}</h4>
                      {configuration.product_id && (
                        <a
                          href={`/product/${encodeURIComponent(configuration.product_id)}`}
                          onClick={(event) => {
                            if (
                              event.button === 0 &&
                              !event.ctrlKey &&
                              !event.metaKey &&
                              !event.shiftKey &&
                              !event.altKey
                            ) {
                              event.preventDefault();
                              onNavigate(
                                `/product/${encodeURIComponent(configuration.product_id!)}`,
                              );
                            }
                          }}
                        >
                          View an offer <ArrowRight size={14} />
                        </a>
                      )}
                    </div>
                    <div className="statistics-config-charts">
                      {configurationChart('lowest_price') && (
                        <StatisticVisual chart={configurationChart('lowest_price')!} />
                      )}
                      {configurationChart('availability') && (
                        <StatisticVisual chart={configurationChart('availability')!} />
                      )}
                    </div>
                  </div>
                )}
              </>
            ) : (
              <p className="statistics-no-config">
                No matching configurations. Try broader filters.
              </p>
            )}
          </div>
        )}
        {error && (
          <div className="statistics-error" role="alert">
            <p>{error}</p>
            <button onClick={() => setRevision((value) => value + 1)}>
              <RefreshCw size={15} /> Try again
            </button>
          </div>
        )}
      </section>
      <div className="statistics-back">
        <a
          href="/"
          onClick={(event) => {
            if (
              event.button === 0 &&
              !event.ctrlKey &&
              !event.metaKey &&
              !event.shiftKey &&
              !event.altKey
            ) {
              event.preventDefault();
              onNavigate('/');
            }
          }}
        >
          Back to discovery <ArrowRight size={16} />
        </a>
      </div>
    </main>
  );
}
