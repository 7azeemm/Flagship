import { useEffect, useRef, useState, type ReactNode } from 'react';
import {
  ArrowRight,
  BarChart3,
  ChevronRight,
  Clock3,
  Info,
  RefreshCw,
  SlidersHorizontal,
  Store,
} from 'lucide-react';
import type { ServerInfo, StatisticChart, StatisticsData } from '../types';
import { parseStatistics, request } from '../lib/api';
import { fieldLabel, money, sectionLabel } from '../lib/catalog';
import {
  historyLabel,
  insightParams,
  insightRanges,
  statisticsAPIParams,
  priceTick,
  readInsightQuery,
  rankRetailers,
  minimumRetailerComparisons,
  shortDate,
  signedPercent,
  type InsightQuery,
  type InsightView,
} from '../lib/insights';
import { MarketTrendChart } from './MarketTrendChart';
import { ShareButton } from './ShareButton';
import { StatisticsSelect } from './StatisticsSelect';
import { StatisticsPriceChanges } from './StatisticsPriceChanges';
import { RetailerLandscape } from './RetailerLandscape';
import { ConfigurationStatistics } from './ConfigurationStatistics';
import { ConfigurationMatches } from './ConfigurationMatches';
import '../statistics.css';
import '../configuration-statistics.css';
import '../retailer-landscape.css';

const statisticsCache = new Map<string, { data: StatisticsData; updated: number }>();
const views: { id: InsightView; name: string; icon: typeof BarChart3 }[] = [
  { id: 'overview', name: 'Overview', icon: BarChart3 },
  { id: 'retailers', name: 'Retailers', icon: Store },
  { id: 'explorer', name: 'Configurations', icon: SlidersHorizontal },
];
const marketMetrics = [
  { metric: 'index', label: 'Price movement' },
  { metric: 'availability', label: 'In-stock rate' },
] as const;
function changeColor(value: number | null | undefined) {
  return value != null && value > 0.05
    ? 'insight-up'
    : value != null && value < -0.05
      ? 'insight-down'
      : '';
}
function valueLabel(chart: StatisticChart, value: number) {
  if (chart.metric === 'availability') return `${value.toFixed(1)}%`;
  if (chart.metric === 'index') return signedPercent(value - 100);
  return `${money(Math.round(value))} DT`;
}
function StatisticVisual({ chart, controls }: { chart: StatisticChart; controls?: ReactNode }) {
  const last = chart.points.at(-1);
  const title =
    chart.metric === 'lowest_price'
      ? 'Lowest price'
      : marketMetrics.find((item) => item.metric === chart.metric)?.label || chart.title;
  return (
    <article className="statistics-card">
      {controls}
      <div className="statistics-card-heading">
        <div>
          <span>{sectionLabel(chart.section)}</span>
          <h3>{title}</h3>
          <p>{chart.description}</p>
        </div>
        {last && (
          <strong className={chart.metric === 'index' ? changeColor(last.value - 100) : ''}>
            {valueLabel(chart, last.value)}
          </strong>
        )}
      </div>
      {chart.points.length > 1 ? (
        <MarketTrendChart
          title={`${title} for ${sectionLabel(chart.section)}`}
          points={chart.points.map((point) => ({
            timestamp: point.timestamp,
            value: chart.metric === 'index' ? point.value - 100 : point.value,
            sampleSize: point.sample_size,
            breakBefore: point.break_before,
          }))}
          valueLabel={(value) =>
            chart.metric === 'index' ? signedPercent(value) : valueLabel(chart, value)
          }
          tickLabel={(value) =>
            chart.metric === 'index'
              ? signedPercent(value)
              : chart.metric === 'availability'
                ? `${value.toFixed(1)}%`
                : priceTick(value)
          }
          baseline={chart.metric === 'index' ? 0 : undefined}
          colorBySign={chart.metric === 'index'}
          minimumPadding={chart.metric === 'index' ? 0.5 : chart.metric === 'availability' ? 2 : 4}
          maximumValue={chart.metric === 'availability' ? 100 : undefined}
        />
      ) : (
        <div className="statistics-empty">
          <BarChart3 size={26} />
          <strong>Not enough history yet</strong>
          <span>
            {chart.metric === 'index'
              ? 'We need five comparable offers across two observations.'
              : chart.metric === 'availability' && chart.id === 'availability'
                ? 'We need two snapshots with at least five eligible offers each.'
                : 'We need at least two supported snapshots.'}
          </span>
        </div>
      )}
      <div className="statistics-card-meta">
        <span>{historyLabel(chart.points)}</span>
        <span>
          {money(chart.metric === 'index' ? chart.comparable_offers : last?.sample_size || 0)}{' '}
          offers in latest {chart.metric === 'index' ? 'comparison' : 'point'}
        </span>
      </div>
      <details>
        <summary>
          <Info size={14} /> How this is calculated
        </summary>
        <p>{chart.note}</p>
      </details>
    </article>
  );
}
function MarketOverview({
  charts,
  metric,
  onMetricChange,
  includeLowestPrice = false,
}: {
  charts: StatisticChart[];
  metric: InsightQuery['metric'];
  onMetricChange: (metric: InsightQuery['metric']) => void;
  includeLowestPrice?: boolean;
}) {
  const selectedMetric =
    metric === 'availability'
      ? 'availability'
      : includeLowestPrice && metric === 'lowest_price'
        ? 'lowest_price'
        : 'index';
  const metrics = includeLowestPrice
    ? [...marketMetrics, { metric: 'lowest_price' as const, label: 'Lowest price' }]
    : marketMetrics;
  const chart = charts.find((item) => item.metric === selectedMetric) || charts[0];
  const controls = (
    <div className="insight-metric-switch" role="group" aria-label="Chart metric">
      {metrics.map((item) => (
        <button
          key={item.metric}
          aria-pressed={selectedMetric === item.metric}
          onClick={() => onMetricChange(item.metric)}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
  if (!chart)
    return (
      <div className="statistics-empty">
        <BarChart3 size={26} />
        <strong>No market observations yet</strong>
        <span>Choose another category or check back after the next scan.</span>
      </div>
    );
  return <StatisticVisual chart={chart} controls={controls} />;
}
function RetailerComparison({
  overview,
  section,
  range,
  measure,
  onMeasureChange,
  onNavigate,
}: {
  overview: StatisticsData['retailer_overviews'][string] | undefined;
  section: string;
  range: string;
  measure: InsightQuery['retailerMeasure'];
  onMeasureChange: (measure: InsightQuery['retailerMeasure']) => void;
  onNavigate: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  if (!overview)
    return (
      <div className="statistics-empty">
        <Store />
        <strong>No retailer observations yet</strong>
      </div>
    );
  const ranked = rankRetailers(overview.retailers, measure);
  const maximum = measure !== 'offers' ? 100 : Math.max(1, ...ranked.map((item) => item.value));
  const priceDataReady = overview.retailers.some((item) => item.within_two_percent !== undefined);
  return (
    <section className="retailer-comparison">
      <div className="insight-section-heading">
        <div>
          <h2>{measure === 'price' ? 'Price ranking' : 'In-stock offer coverage'}</h2>
          <p>
            {measure === 'price'
              ? 'Competitive prices, ranked with more weight given to stronger samples.'
              : 'Retailers ranked by the number of individual products currently in stock.'}
          </p>
        </div>
        <div className="retailer-measures" role="group" aria-label="Retailer measure">
          <button aria-pressed={measure === 'price'} onClick={() => onMeasureChange('price')}>
            Price ranking
          </button>
          <button aria-pressed={measure === 'offers'} onClick={() => onMeasureChange('offers')}>
            Offer coverage
          </button>
        </div>
      </div>
      <div className="insight-retailer-context">
        <span>{measure === 'price' ? 'Current prices' : 'In-stock products only'}</span>
        <span>
          {measure === 'price'
            ? `${money(overview.comparable_configurations)} shared configurations · ${sectionLabel(section)}`
            : `${money(ranked.reduce((total, item) => total + item.value, 0))} products in stock · ${sectionLabel(section)}`}
        </span>
        {measure !== 'offers' && (
          <span>Retailers with {minimumRetailerComparisons}+ comparisons</span>
        )}
      </div>
      {measure === 'price' && !priceDataReady ? (
        <div className="statistics-empty">
          <strong>Retailer price comparison data is not available yet.</strong>
        </div>
      ) : (
        <div className="retailer-bars">
          {(expanded ? ranked : ranked.slice(0, 8)).map((item, index) => (
            <div className="retailer-bar-row" key={item.name}>
              <span className="retailer-bar-rank">{String(index + 1).padStart(2, '0')}</span>
              <a
                href={`/statistics?${new URLSearchParams({ view: 'explorer', section, range, retailer: item.name })}`}
                onClick={(event) => {
                  if (
                    event.button ||
                    event.ctrlKey ||
                    event.metaKey ||
                    event.shiftKey ||
                    event.altKey
                  )
                    return;
                  event.preventDefault();
                  onNavigate(event.currentTarget.pathname + event.currentTarget.search);
                }}
              >
                <strong>{item.name}</strong>
              </a>
              <span className="retailer-bar-track" aria-hidden="true">
                <span style={{ width: `${(item.value / maximum) * 100}%` }} />
              </span>
              <span className="retailer-bar-value">
                {measure !== 'offers' ? `${item.value.toFixed(1)} / 100` : money(item.value)}
                <small>
                  {measure === 'price'
                    ? `${item.competitiveShare.toFixed(0)}% within 2% · ${item.price_comparisons} comparisons`
                    : 'in-stock products'}
                </small>
              </span>
            </div>
          ))}
        </div>
      )}
      {!ranked.length && (
        <div className="statistics-empty">
          <Store />
          <strong>
            {measure !== 'offers'
              ? 'No retailers have five shared comparisons yet.'
              : 'No retailers have in-stock products yet.'}
          </strong>
          <span>
            {measure !== 'offers'
              ? 'Choose another category or view offer coverage.'
              : 'Choose another category.'}
          </span>
        </div>
      )}
      <div className="retailer-comparison-foot">
        <span>
          {ranked.length} retailers match
          {!expanded && ranked.length > 8 ? ' · top eight shown' : ''}
        </span>
        {ranked.length > 8 && (
          <button onClick={() => setExpanded((value) => !value)}>
            {expanded ? 'Show top eight' : `Show all ${ranked.length}`}
          </button>
        )}
      </div>
      <details className="insight-method">
        <summary>
          <Info size={14} /> Understand this comparison
        </summary>
        <p>
          Price ranking rewards stores whose cheapest in-stock offer is the lowest or within 2% of
          the lowest price for a matching configuration. For example, 1,020 DT qualifies when the
          lowest price is 1,000 DT. Each configuration needs at least two retailers to compare.
        </p>
        <p>
          The purple bar shows a score out of 100, adjusted for sample size. A retailer with 19
          qualifying prices out of 22 comparisons scores 66.7; a retailer with 5 out of 5 scores
          56.6. More comparisons give a strong result more weight. Retailers need at least
          {minimumRetailerComparisons} comparisons to appear. The percentage below the score is the
          actual share cheapest or within 2%.
        </p>
        <p>
          The score uses the lower bound of a 95% Wilson interval. Each retailer contributes its
          cheapest in-stock offer per configuration, using the category’s grouping fields. Product
          mixes and sample sizes differ between retailers. Offer coverage counts individual products
          marked in stock, including multiple listings with the same specifications. On-demand,
          arriving, and out-of-stock products are excluded.
        </p>
      </details>
    </section>
  );
}
export function StatisticsPage({
  categories,
  onNavigate,
}: {
  categories: ServerInfo['categories'];
  onNavigate: (path: string) => void;
}) {
  const [query, setQuery] = useState(() => readInsightQuery(window.location.search));
  const [data, setData] = useState<StatisticsData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const refinementRef = useRef<HTMLDivElement>(null);
  const refinementFocus = useRef<string | null>(null);
  useEffect(() => {
    if (query.view !== 'explorer') {
      refinementFocus.current = null;
      return;
    }
    if (!data || !refinementFocus.current) return;
    if (document.activeElement === document.body) {
      const field = [
        ...(refinementRef.current?.querySelectorAll<HTMLDivElement>('.statistics-select') || []),
      ].find((element) => element.dataset.label === refinementFocus.current);
      field
        ?.querySelector<HTMLButtonElement>('.statistics-select-trigger')
        ?.focus({ preventScroll: true });
    }
    refinementFocus.current = null;
  }, [data, query.view]);
  useEffect(() => {
    document.title = 'Market statistics — Flagship';
  }, []);
  useEffect(() => {
    const restore = () => {
      if (window.location.pathname === '/statistics') {
        const next = readInsightQuery(window.location.search);
        setQuery(next);
      }
    };
    window.addEventListener('popstate', restore);
    return () => window.removeEventListener('popstate', restore);
  }, []);
  const change = (patch: Partial<InsightQuery>) => {
    const next = { ...query, ...patch };
    if (next.section === 'All') next.filters = {};
    setQuery(next);
    window.history.pushState(null, '', `/statistics?${insightParams(next)}`);
  };
  const path = `/stats?${statisticsAPIParams(query)}`;
  useEffect(() => {
    const controller = new AbortController();
    const cached = statisticsCache.get(path);
    setError('');
    if (cached && revision === 0 && Date.now() - cached.updated < 30_000) {
      setData(cached.data);
      setLoading(false);
      return;
    }
    setData(null);
    setLoading(true);
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
  }, [path, revision]);
  const sections = [
    ...new Set(
      Object.values(categories)
        .flat()
        .map((category) => category.id),
    ),
  ].filter((id) =>
    data
      ? data.supported_sections.includes(id)
      : [
          'PC',
          'GamingPC',
          'AllInOnePC',
          'MiniPC',
          'Laptop',
          'GamingLaptop',
          'MacBook',
          'CPU',
          'GPU',
          'Smartphone',
        ].includes(id),
  );
  const charts = data?.explorer_charts || [];
  const index = charts.find((chart) => chart.metric === 'index');
  const priceChanges = data?.price_changes || null;
  const stock = charts.find((chart) => chart.metric === 'availability');
  const overviewRef = useRef<HTMLDivElement>(null);
  const revealOverview = useRef(false);
  useEffect(() => {
    if (data && revealOverview.current && query.view === 'overview') {
      overviewRef.current?.scrollIntoView({ block: 'start' });
      overviewRef.current?.focus({ preventScroll: true });
      revealOverview.current = false;
    }
  }, [data, query.view]);
  const activeFilters = Object.keys(query.filters).length + Number(!!query.retailer);
  return (
    <main
      className="statistics-main insights-page statistics-dashboard"
      id="statistics-content"
      tabIndex={-1}
    >
      <header className="insights-header">
        <div>
          <h1>Market statistics</h1>
          <p className="insight-updated">
            <Clock3 size={13} />
            {data ? (
              <>
                {data.last_scan_at ? 'Last scan' : 'Snapshot'}{' '}
                <time dateTime={data.last_scan_at || data.generated_at}>
                  {new Date(data.last_scan_at || data.generated_at).toLocaleString('en-GB', {
                    day: 'numeric',
                    month: 'short',
                    hour: '2-digit',
                    minute: '2-digit',
                  })}
                </time>
              </>
            ) : (
              'Recorded prices across Tunisian retailers'
            )}
          </p>
        </div>
        <ShareButton
          title="Market statistics — Flagship"
          path={`/statistics?${insightParams(query)}`}
        />
      </header>
      <nav className="insight-tabs" aria-label="Statistics views">
        {views.map(({ id, name, icon: Icon }) => (
          <button
            key={id}
            aria-current={query.view === id ? 'page' : undefined}
            onClick={() => change({ view: id })}
          >
            <Icon size={16} />
            {name}
          </button>
        ))}
      </nav>
      <div className="insight-toolbar">
        <label>
          Category
          <select
            aria-label="Category"
            value={query.section}
            onChange={(event) => {
              change({
                section: event.target.value,
                filters: {},
                retailer: '',
              });
            }}
          >
            <option value="All">All categories</option>
            {query.section !== 'All' && !sections.includes(query.section) && (
              <option value={query.section}>{sectionLabel(query.section)}</option>
            )}
            {sections.map((section) => (
              <option key={section} value={section}>
                {sectionLabel(section)}
              </option>
            ))}
          </select>
        </label>
        {query.view !== 'retailers' && (
          <div className="insight-period-control">
            <span id="insight-period-label">Period</span>
            <div className="insight-periods" role="group" aria-labelledby="insight-period-label">
              {insightRanges.map(([value, label]) => (
                <button
                  key={value}
                  aria-pressed={query.range === value}
                  onClick={() => change({ range: value })}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        )}
        <span className="insight-toolbar-context">
          {query.view === 'retailers'
            ? 'Current prices and stock'
            : query.view === 'explorer'
              ? 'History uses this period · offer summaries are current'
              : 'Recorded history within this period'}
        </span>
      </div>
      {query.view === 'explorer' && (
        <div className="insight-refinements">
          <h2 className="insight-refine-heading">
            <SlidersHorizontal size={16} />{' '}
            {query.section === 'All' ? 'Retailer' : 'Retailer & specifications'}{' '}
            {activeFilters ? `(${activeFilters} active)` : ''}
          </h2>
          <div id="insight-refine-body" className="statistics-refine-body" ref={refinementRef}>
            <StatisticsSelect
              label="Retailer"
              value={query.retailer}
              options={[
                { value: '', label: 'All retailers' },
                ...(query.retailer && !data?.retailers.includes(query.retailer)
                  ? [{ value: query.retailer, label: query.retailer }]
                  : []),
                ...(data?.retailers.map((retailer) => ({ value: retailer, label: retailer })) ||
                  []),
              ]}
              onChange={(retailer) => {
                refinementFocus.current = 'Retailer';
                change({ retailer });
              }}
            />
            {query.section !== 'All' &&
              data?.facets.map((facet) => (
                <StatisticsSelect
                  key={facet.key}
                  label={fieldLabel(facet.key)}
                  value={query.filters[facet.key] || ''}
                  options={[
                    { value: '', label: 'Any' },
                    ...facet.options.map((option) => ({
                      value: option.value,
                      label: option.value,
                      count: option.count,
                      disabled: option.count === 0 && query.filters[facet.key] !== option.value,
                    })),
                  ]}
                  onChange={(value) => {
                    refinementFocus.current = fieldLabel(facet.key);
                    const filters = { ...query.filters };
                    if (value) filters[facet.key] = value;
                    else delete filters[facet.key];
                    change({ filters });
                  }}
                />
              ))}
          </div>
          {activeFilters > 0 && (
            <div className="insight-active-filters">
              {query.retailer && <span>{query.retailer}</span>}
              {Object.entries(query.filters).map(([key, value]) => (
                <span key={key}>
                  {fieldLabel(key)}: {value}
                </span>
              ))}
              <button onClick={() => change({ retailer: '', filters: {} })}>
                Clear refinements
              </button>
            </div>
          )}
        </div>
      )}
      {error && (
        <div className="statistics-error" role="alert">
          <strong>We couldn’t load this view.</strong>
          <p>{error}</p>
          <button onClick={() => setRevision((value) => value + 1)}>
            <RefreshCw size={15} /> Try again
          </button>
        </div>
      )}
      {loading && (
        <div className="insight-loading" role="status">
          <span />
          <span />
          <span />
          <p>Loading market observations…</p>
        </div>
      )}
      {data && query.view === 'retailers' && (
        <>
          <RetailerComparison
            key={`comparison-${query.section}`}
            section={query.section}
            range={query.range}
            overview={data.retailer_overviews[query.section]}
            measure={query.retailerMeasure}
            onMeasureChange={(retailerMeasure) => change({ retailerMeasure })}
            onNavigate={onNavigate}
          />
          <RetailerLandscape
            key={`landscape-${query.section}`}
            overview={data.retailer_overviews[query.section]}
            section={query.section}
            range={query.range}
            onNavigate={onNavigate}
          />
        </>
      )}
      {data && query.view === 'overview' && (
        <>
          <div className="insight-summary">
            <article>
              <span>Price change</span>
              <strong
                className={changeColor(
                  index?.points.at(-1) ? index.points.at(-1)!.value - 100 : null,
                )}
              >
                {index?.points.length ? signedPercent(index.points.at(-1)!.value - 100) : '—'}
              </strong>
              <small>
                {index?.points.length
                  ? `Comparable offers · since ${shortDate(index.points[0].timestamp)}`
                  : 'Collecting comparable history'}
              </small>
            </article>
            <article>
              <span>In-stock rate</span>
              <strong>
                {stock?.points.length ? `${stock.points.at(-1)!.value.toFixed(1)}%` : '—'}
              </strong>
              <small>
                {money(index?.matching_offers || 0)} matching offers · {index?.retailer_count || 0}{' '}
                {index?.retailer_count === 1 ? 'retailer' : 'retailers'}
              </small>
            </article>
            <article>
              <span>Retailers tracked</span>
              <strong>{index?.retailer_count || 0}</strong>
              <small>{sectionLabel(query.section)} · current coverage</small>
            </article>
          </div>
          <div
            className="insight-main-chart"
            ref={overviewRef}
            tabIndex={-1}
            role="region"
            aria-label={`${sectionLabel(query.section)} market trend`}
          >
            <MarketOverview
              charts={charts}
              metric={query.metric}
              onMetricChange={(metric) => change({ metric })}
            />
          </div>
          <section className="insight-price-changes" aria-label="Price changes during this period">
            <StatisticsPriceChanges data={priceChanges} />
          </section>
          <section className="insight-category-section">
            <div className="insight-section-heading">
              <div>
                <h2>Compare categories</h2>
                <p>Price movements within this period. Select a row to explore its market.</p>
              </div>
              <button className="insight-text-action" onClick={() => change({ view: 'retailers' })}>
                Compare retailers <ArrowRight size={15} />
              </button>
            </div>
            <div className="insight-category-table">
              <table aria-label="Category price comparisons">
                <thead>
                  <tr>
                    <th scope="col">Category</th>
                    <th scope="col">Price change</th>
                    <th scope="col">Compared offers</th>
                  </tr>
                </thead>
                <tbody>
                  {data.category_trends.map((trend) => {
                    const selectCategory = () => {
                      revealOverview.current = trend.section !== query.section;
                      if (!revealOverview.current)
                        overviewRef.current?.scrollIntoView({ block: 'start' });
                      change({
                        section: trend.section,
                        filters: {},
                        retailer: '',
                      });
                    };
                    return (
                      <tr
                        key={trend.section}
                        className={trend.section === query.section ? 'is-selected' : ''}
                        onClick={selectCategory}
                      >
                        <th scope="row">
                          <button
                            aria-pressed={trend.section === query.section}
                            onClick={(event) => {
                              event.stopPropagation();
                              selectCategory();
                            }}
                          >
                            <span>
                              {sectionLabel(trend.section)}
                              <small>
                                {trend.since
                                  ? `Since ${shortDate(trend.since)}`
                                  : 'Collecting history'}
                              </small>
                            </span>
                            <ChevronRight size={15} />
                          </button>
                        </th>
                        <td>
                          <strong className={changeColor(trend.change_percent)}>
                            {trend.change_percent == null
                              ? '—'
                              : signedPercent(trend.change_percent)}
                          </strong>
                        </td>
                        <td>{money(trend.comparable_offers)}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}
      {data && query.view === 'explorer' && (
        <>
          <ConfigurationStatistics
            data={data.offer_summary}
            section={query.section}
            lowestPrice={
              data.offer_summary?.available_priced_offers
                ? (charts.find((chart) => chart.metric === 'lowest_price')?.points.at(-1)?.value ??
                  null)
                : null
            }
          />
          {(data.offer_summary?.offer_count || 0) > 0 && (
            <section className="configuration-history" aria-label="History for matching offers">
              <div className="insight-section-heading">
                <div>
                  <h2>Matching offers over time</h2>
                  <p>
                    Price movement, in-stock rate and lowest in-stock prices for your selected
                    filters.
                  </p>
                </div>
              </div>
              <MarketOverview
                charts={charts}
                metric={query.metric}
                includeLowestPrice
                onMetricChange={(metric) => change({ metric })}
              />
            </section>
          )}
          <ConfigurationMatches
            products={data.matching_products}
            section={query.section}
            retailer={query.retailer}
            filters={query.filters}
            catalogFilters={data.catalog_filters}
            onNavigate={onNavigate}
          />
          {(index?.matching_offers || 0) > 0 && (
            <section
              className="configuration-period-changes"
              aria-label="Price changes for selected specifications"
            >
              <StatisticsPriceChanges data={priceChanges} />
            </section>
          )}
        </>
      )}
      <p className="insight-page-note">
        <Info size={14} /> Based on recorded offers in categories that support grouping. Prices and
        stock may change between scans.
      </p>
    </main>
  );
}
