import { useEffect, useState } from 'react';
import {
  ArrowDown,
  ArrowRight,
  ArrowUp,
  PackageSearch,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  Store,
  TrendingUp,
} from 'lucide-react';
import type { HomeProduct, HomepageData } from '../types';
import { parseHomepage, request } from '../lib/api';
import { money, sectionLabel } from '../lib/catalog';
import { MarketTrendChart } from './MarketTrendChart';
import '../homepage.css';

const ranges = [
  ['7d', '7D'],
  ['30d', '30D'],
  ['90d', '3M'],
  ['180d', '6M'],
  ['1y', '1Y'],
  ['all', 'All'],
] as const;
const categorySections = ['GamingLaptop', 'GPU', 'CPU', 'Smartphone', 'Monitor'];
let homeCache: { data: HomepageData; updated: number } | null = null;

function relativeTime(value: string | null, now = Date.now()) {
  if (!value) return 'at an unknown time';
  const difference = now - Date.parse(value);
  if (!Number.isFinite(difference)) return 'at an unknown time';
  const formatter = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['year', 31_536_000_000],
    ['month', 2_592_000_000],
    ['day', 86_400_000],
    ['hour', 3_600_000],
    ['minute', 60_000],
  ];
  for (const [unit, size] of units) {
    if (Math.abs(difference) >= size) return formatter.format(-Math.round(difference / size), unit);
  }
  return 'just now';
}

function productPath(product: HomeProduct) {
  return `/product/${encodeURIComponent(product.id)}`;
}

function ProductImage({ product }: { product: HomeProduct }) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const failed = failedSource === product.image;
  return (
    <span className={`home-product-image ${failed || !product.image ? 'image-unavailable' : ''}`}>
      {product.image && !failed ? (
        <img
          src={product.image}
          alt={product.title}
          loading="lazy"
          onError={() => setFailedSource(product.image)}
        />
      ) : (
        <PackageSearch aria-hidden="true" />
      )}
    </span>
  );
}

function Price({ value }: { value: number }) {
  return (
    <>
      {money(value)} <small>DT</small>
    </>
  );
}

function TrendValue({ value }: { value: number | null }) {
  if (value === null) return <span className="home-trend neutral">Collecting data</span>;
  const down = value < -0.05;
  const up = value > 0.05;
  return (
    <span className={`home-trend ${down ? 'down' : up ? 'up' : 'neutral'}`}>
      {down ? <ArrowDown size={14} /> : up ? <ArrowUp size={14} /> : null}
      {Math.abs(value).toFixed(1)}%
    </span>
  );
}

function MarketChart({ points }: { points: HomepageData['market_views'][number]['points'] }) {
  if (points.length < 2) {
    return (
      <div className="market-chart-empty">
        <TrendingUp size={25} />
        <strong>Building the first market trend</strong>
        <span>More comparable price observations are needed for this period.</span>
      </div>
    );
  }
  return (
    <MarketTrendChart
      title="Price Index change over time, in percent"
      points={points.map((point) => ({
        timestamp: point.timestamp,
        value: point.index - 100,
        sampleSize: point.comparable_products,
      }))}
      valueLabel={(value) => `${value > 0 ? '+' : ''}${value.toFixed(1)}%`}
      tickLabel={(value) => `${Math.abs(value) < 0.05 ? '0.0' : value.toFixed(1)}%`}
      baseline={0}
      minimumPadding={0.5}
    />
  );
}

function DealCard({
  product,
  onNavigate,
}: {
  product: HomeProduct;
  onNavigate: (path: string) => void;
}) {
  const href = productPath(product);
  return (
    <a
      className="deal-card"
      href={href}
      onClick={(event) => {
        if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        onNavigate(href);
      }}
    >
      <div className="deal-card-top">
        <span>{product.badge || 'Price drop'}</span>
        {product.change_percent !== null && <TrendValue value={product.change_percent} />}
      </div>
      <ProductImage product={product} />
      <span className="home-product-section">{sectionLabel(product.section)}</span>
      <h3>{product.title}</h3>
      <div className="deal-price">
        <strong>
          <Price value={product.price} />
        </strong>
      </div>
      <div className="deal-saving">
        {product.previous_price !== null && (
          <span>
            Was <del>{money(product.previous_price)} DT</del>
          </span>
        )}
        {product.saving !== null && <strong>Down {money(product.saving)} DT</strong>}
      </div>
      <span className="deal-retailer">
        At {product.site}{' '}
        <span>
          View price history <ArrowRight size={13} />
        </span>
      </span>
    </a>
  );
}

export function HomePage({
  onNavigate,
  onAbout,
}: {
  onNavigate: (path: string) => void;
  onAbout: () => void;
}) {
  const [range, setRange] = useState('30d');
  const [section, setSection] = useState('All');
  const [data, setData] = useState<HomepageData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const clock = window.setInterval(() => setNow(Date.now()), 30_000);
    const refresh = window.setInterval(() => setRevision((value) => value + 1), 300_000);
    return () => {
      window.clearInterval(clock);
      window.clearInterval(refresh);
    };
  }, []);

  useEffect(() => {
    document.title = 'Mythos — Tech prices in perspective';
  }, []);
  useEffect(() => {
    const controller = new AbortController();
    if (homeCache && Date.now() - homeCache.updated < 270_000) {
      setData(homeCache.data);
      setLoading(false);
      setError('');
      return () => controller.abort();
    }
    setLoading(true);
    setError('');
    request('/homepage', controller.signal)
      .then(parseHomepage)
      .then((result) => {
        if (!controller.signal.aborted) {
          homeCache = { data: result, updated: Date.now() };
          setData(result);
        }
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'The homepage could not be loaded.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [revision]);

  if (!data && loading)
    return (
      <main className="home-main" id="home-content" tabIndex={-1} aria-busy="true">
        <div className="home-hero-skeleton" aria-label="Loading homepage" />
      </main>
    );
  if (!data && error) {
    return (
      <main className="home-main" id="home-content" tabIndex={-1}>
        <div className="home-load-error" role="alert">
          <RefreshCw size={28} />
          <h1>We couldn’t load the market view.</h1>
          <p>{error}</p>
          <button className="primary-button" onClick={() => setRevision((value) => value + 1)}>
            Try again <ArrowRight size={16} />
          </button>
        </div>
      </main>
    );
  }
  if (!data) return null;

  const featured = data.hero_deal || data.new_products[0] || null;
  const visibleDeals = data.deals.filter((product) => product.id !== featured?.id).slice(0, 4);
  const market =
    data.market_views.find((view) => view.range === range && (view.section || 'All') === section) ??
    data.market_views[0]!;
  const change = market.change_percent;
  const period = ranges.find(([value]) => value === market.range)?.[1] || '30D';
  const marketSubject = !market.section
    ? 'Tech prices'
    : `Prices for ${sectionLabel(market.section).toLowerCase()}`;
  const marketLead =
    change === null
      ? `Mythos is building a comparable trend for ${!market.section ? 'the market' : sectionLabel(market.section).toLowerCase()}.`
      : `${marketSubject} are ${Math.abs(change) < 0.05 ? 'holding steady' : change < 0 ? `down ${Math.abs(change).toFixed(1)}%` : `up ${change.toFixed(1)}%`} over ${period.toLowerCase()}.`;

  const go = (event: React.MouseEvent<HTMLAnchorElement>, path: string) => {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
      return;
    event.preventDefault();
    onNavigate(path);
  };

  return (
    <main className="home-main" id="home-content" tabIndex={-1} aria-busy={loading}>
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-hero-copy">
          <span className="home-eyebrow">
            <Sparkles size={14} /> Tunisia’s tech market, made clearer
          </span>
          <h1 id="home-title">
            Know when the <span>price is right.</span>
          </h1>
          <p>
            Compare offers across Tunisian retailers, check their price history, and find the right
            moment to buy.
          </p>
          <div className="home-hero-actions">
            <a href="#deals" className="primary-button">
              Explore today’s deals <ArrowRight size={17} />
            </a>
            <button className="secondary-button" onClick={onAbout}>
              How Mythos works
            </button>
          </div>
          <div className="home-coverage" aria-label="Catalog coverage">
            <span>
              <strong>{money(data.coverage.products)}</strong> product listings
            </span>
            <i />
            <span>
              <strong>{data.coverage.retailers}</strong> retailers
            </span>
            <i />
            <span title={data.coverage.last_scan_at || data.coverage.updated_at || undefined}>
              {data.coverage.last_scan_at
                ? `Last scan ${relativeTime(data.coverage.last_scan_at, now)}`
                : data.coverage.updated_at
                  ? `Latest catalog change ${relativeTime(data.coverage.updated_at, now)}`
                  : 'Scan time unavailable'}
            </span>
            {data.coverage.next_scan_at && (
              <>
                <i />
                <span title={data.coverage.next_scan_at}>
                  Next scan expected {relativeTime(data.coverage.next_scan_at, now)}
                </span>
              </>
            )}
          </div>
        </div>
        {featured && (
          <a
            className="hero-product"
            href={productPath(featured)}
            onClick={(event) => go(event, productPath(featured))}
          >
            <div className="hero-product-glow" />
            <div className="hero-product-label">
              <ShieldCheck size={15} /> {data.hero_deal ? featured.badge : 'New on Mythos'}
            </div>
            <ProductImage product={featured} />
            <div className="hero-product-copy">
              <span>
                {sectionLabel(featured.section)} · {featured.site}
              </span>
              <h2>{featured.title}</h2>
              <div>
                <strong>
                  <Price value={featured.price} />
                </strong>
                {featured.previous_price !== null && <del>{money(featured.previous_price)} DT</del>}
              </div>
              <p className="hero-price-context">
                {featured.saving !== null
                  ? `${money(featured.saving)} DT below its earlier tracked price${featured.change_percent !== null ? ` · ${Math.abs(featured.change_percent).toFixed(1)}% lower` : ''}.`
                  : 'A recent addition to the Mythos catalog.'}
              </p>
              <b>
                See the product <ArrowRight size={15} />
              </b>
            </div>
          </a>
        )}
      </section>

      {error && (
        <div className="home-inline-error" role="alert">
          <span>Couldn’t update the market data. {error}</span>
          <button onClick={() => setRevision((value) => value + 1)}>Try again</button>
        </div>
      )}

      <section className="home-section" id="deals" aria-labelledby="deals-heading">
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">Recorded price drops</span>
            <h2 id="deals-heading">Deals worth a look</h2>
            <p>Current offers with a price drop recorded in the past 30 days.</p>
          </div>
        </div>
        {loading && (
          <span className="home-updating" role="status">
            Updating prices…
          </span>
        )}
        {visibleDeals.length ? (
          <div className="deal-grid">
            {visibleDeals.map((product) => (
              <DealCard key={product.id} product={product} onNavigate={onNavigate} />
            ))}
          </div>
        ) : (
          <div className="home-empty">
            <ShieldCheck />
            <strong>
              {data.deals.length
                ? 'The featured offer is the only drop here'
                : 'No recorded drops for this period'}
            </strong>
            <span>Explore newly discovered products below.</span>
          </div>
        )}
      </section>

      <section className="home-market" aria-labelledby="market-heading">
        <div className="home-section-heading market-heading-row">
          <div>
            <span className="home-kicker">The market at a glance</span>
            <h2 id="market-heading">{marketLead}</h2>
          </div>
          <div className="home-range" aria-label="Market period">
            {ranges.map(([value, label]) => (
              <button
                key={value}
                className={range === value ? 'active' : ''}
                aria-pressed={range === value}
                onClick={() => setRange(value)}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        {loading && (
          <span className="home-updating" role="status">
            Updating trend…
          </span>
        )}
        <div className="market-layout">
          <div className="market-visual">
            <div className="market-index-row">
              <div>
                <span>Price Index</span>
                <strong>
                  {change === null ? '—' : `${change > 0 ? '+' : ''}${change.toFixed(1)}%`}
                </strong>
              </div>
              <span className="market-period-label">{period}</span>
            </div>
            <MarketChart points={market.points} />
            <p className="market-method">
              The trend compares offers with valid prices at both ends of each interval. The latest
              interval includes {money(market.comparable_products)} of{' '}
              {money(market.tracked_offers)}
              tracked offers in this view. <button onClick={onAbout}>
                How it’s calculated
              </button>{' '}
              <a href="/statistics" onClick={(event) => go(event, '/statistics')}>
                Explore statistics <ArrowRight size={12} />
              </a>
            </p>
          </div>
          <div className="market-facts">
            <article>
              <Store />
              <span>
                <strong>{market.in_stock_percent.toFixed(0)}%</strong> offers in stock now
                <small>Across the selected market view</small>
              </span>
            </article>
            <article>
              <TrendingUp />
              <span>
                <strong>{money(market.comparable_products)}</strong> offers compared
                <small>With prices at both ends of the latest interval</small>
              </span>
            </article>
            <div className="market-moves">
              <article>
                <ArrowDown />
                <span>
                  <strong>{money(market.price_decreases)}</strong> price drops
                  <small>Recorded in this period</small>
                </span>
              </article>
              <article>
                <ArrowUp />
                <span>
                  <strong>{money(market.price_increases)}</strong> price rises
                  <small>Recorded in this period</small>
                </span>
              </article>
            </div>
          </div>
        </div>
        <div className="market-categories" aria-label="Market category">
          <button className={section === 'All' ? 'active' : ''} onClick={() => setSection('All')}>
            All products
          </button>
          {categorySections.map((item) => {
            const movement =
              data.market_views.find((view) => view.range === range && view.section === item)
                ?.change_percent ?? null;
            return (
              <button
                key={item}
                className={section === item ? 'active' : ''}
                onClick={() => setSection(item)}
              >
                {sectionLabel(item)} <TrendValue value={movement} />
              </button>
            );
          })}
        </div>
      </section>

      <section className="home-discovery" aria-label="Product discovery">
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">Recently discovered</span>
            <h2>New on Mythos</h2>
            <p>Fresh additions from across the catalog.</p>
          </div>
        </div>
        <div className="new-product-list">
          {data.new_products
            .filter((product) => product.id !== featured?.id)
            .slice(0, 8)
            .map((product) => (
              <a
                key={product.id}
                href={productPath(product)}
                onClick={(event) => go(event, productPath(product))}
              >
                <ProductImage product={product} />
                <div>
                  <strong>{product.title}</strong>
                  <span>
                    {money(product.price)} DT · {product.site}
                  </span>
                  <small title={product.added_at}>{relativeTime(product.added_at, now)}</small>
                </div>
                <ArrowRight />
              </a>
            ))}
        </div>
      </section>

      <section className="home-explore" aria-labelledby="explore-heading">
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">A place to start</span>
            <h2 id="explore-heading">Browse by category</h2>
            <p>Open a category to compare current offers and specifications.</p>
          </div>
        </div>
        <div className="home-explore-grid">
          {categorySections.map((item) => {
            const view = data.market_views.find(
              (candidate) => candidate.range === '30d' && candidate.section === item,
            );
            const href = `/${item}`;
            return (
              <a key={item} href={href} onClick={(event) => go(event, href)}>
                <span>{sectionLabel(item)}</span>
                <strong>{money(view?.tracked_offers || 0)} offers</strong>
                <small>
                  Explore category <ArrowRight size={14} />
                </small>
              </a>
            );
          })}
        </div>
      </section>

      <section className="home-retailers" aria-labelledby="retailers-heading">
        <div className="retailer-copy">
          <span className="home-kicker">Coverage across Tunisia</span>
          <h2 id="retailers-heading">More stores. One clearer view.</h2>
          <p>
            Mythos follows {money(data.coverage.in_stock_offers)} currently available offers across{' '}
            {data.coverage.retailers} retailers.
          </p>
          <span className="retailer-trust">
            <ShieldCheck size={16} /> Historical context, independent of retailer discount labels
          </span>
        </div>
        <div className="retailer-grid">
          {data.retailers.map((retailer) => (
            <div key={retailer.name} className="retailer-tile">
              <span>{retailer.name.slice(0, 2).toUpperCase()}</span>
              <div>
                <strong>{retailer.name}</strong>
                <small>
                  {money(retailer.products)} listings · {money(retailer.in_stock)} in stock
                </small>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="home-trust" aria-label="Why use Mythos">
        <span>
          <ShieldCheck /> Price history, not hype
        </span>
        <span>
          <TrendingUp /> Market-wide perspective
        </span>
        <span>
          <Store /> {data.coverage.retailers} retailers tracked
        </span>
      </section>
    </main>
  );
}
