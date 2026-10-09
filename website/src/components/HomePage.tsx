import { useEffect, useRef, useState } from 'react';
import {
  ArrowRight,
  Bell,
  Cpu,
  Gamepad2,
  Laptop,
  Monitor,
  PackageSearch,
  RefreshCw,
  ShieldCheck,
  Store,
  Clock3,
  ChevronLeft,
  ChevronRight,
  Smartphone,
  Sparkles,
  TrendingUp,
} from 'lucide-react';
import type { HomeProduct, HomepageData, ServerInfo } from '../types';
import { parseHomepage, request } from '../lib/api';
import { money, sectionLabel } from '../lib/catalog';
import { historyLabel, signedPercent } from '../lib/insights';
import { MarketTrendChart } from './MarketTrendChart';
import { ProductImage } from './DiscoveryCard';
import { addedAgo, retailerDiscount } from '../lib/homepage';
import '../homepage.css';

let homeCache: { data: HomepageData; updated: number } | null = null;
const categoryIcons = {
  Laptop,
  GamingLaptop: Laptop,
  GamingPC: Gamepad2,
  Smartphone,
  GPU: Monitor,
  CPU: Cpu,
  Monitor,
};
const shortcuts = ['Laptop', 'GamingLaptop', 'GamingPC', 'Smartphone', 'GPU', 'CPU', 'Monitor'];

export function HomePage({
  onNavigate,
  categories,
}: {
  onNavigate: (path: string) => void;
  categories: ServerInfo['categories'];
}) {
  const [data, setData] = useState<HomepageData | null>(homeCache?.data || null);
  const [loading, setLoading] = useState(!homeCache);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const [now, setNow] = useState(Date.now);
  const [allRetailers, setAllRetailers] = useState(false);
  const [marketSection, setMarketSection] = useState('All');
  const [allCategories, setAllCategories] = useState(false);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    document.title = 'Flagship — Tech prices in perspective';
  }, []);
  useEffect(() => {
    const refresh = window.setInterval(() => setRevision((value) => value + 1), 300_000);
    return () => window.clearInterval(refresh);
  }, []);
  useEffect(() => {
    const controller = new AbortController();
    if (homeCache && Date.now() - homeCache.updated < 270_000 && revision === 0) {
      setData(homeCache.data);
      setLoading(false);
      return;
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
          setError(cause instanceof Error ? cause.message : 'Could not load discovery.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [revision]);
  const go = (event: React.MouseEvent<HTMLAnchorElement>) => {
    if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    onNavigate(event.currentTarget.pathname + event.currentTarget.search);
  };
  if (!data)
    return (
      <main className="home-main" id="home-content" tabIndex={-1} aria-busy={loading}>
        {error ? (
          <div className="home-load-error" role="alert">
            <RefreshCw />
            <h1>Discovery is temporarily unavailable.</h1>
            <p>{error}</p>
            <button className="primary-button" onClick={() => setRevision((value) => value + 1)}>
              Try again
            </button>
          </div>
        ) : (
          <div className="home-hero-skeleton" aria-label="Loading homepage" />
        )}
      </main>
    );
  const featured =
    data.hero_deal && retailerDiscount(data.hero_deal)
      ? data.hero_deal
      : data.deals.find((product) => retailerDiscount(product));
  const featuredDiscount = featured ? retailerDiscount(featured) : null;
  const available = Object.values(categories).flat();
  const primaryCategories = shortcuts.filter((id) =>
    available.some((category) => category.id === id),
  );
  const market =
    data.market_views.find(
      (view) => view.range === '30d' && (view.section || 'All') === marketSection,
    ) || data.market_views[0];
  const marketHref = `/statistics?view=overview&section=${encodeURIComponent(market.section || 'GamingLaptop')}&range=30d`;
  const updated = data.coverage.last_scan_at || data.coverage.updated_at;

  return (
    <main className="home-main home-refresh" id="home-content" tabIndex={-1} aria-busy={loading}>
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-hero-copy">
          <span className="home-eyebrow">
            <Sparkles size={14} /> Tunisia’s tech market, made clearer
          </span>
          <h1 id="home-title">
            Know when the <span>price is right.</span>
          </h1>
          <p>
            Find your next upgrade. Compare Tunisian retailers, check price history, and let the
            right offer come to you.
          </p>
          <div className="home-hero-actions">
            <a href="#categories" className="primary-button">
              Browse products <ArrowRight size={17} />
            </a>
            <a href="#deals" className="secondary-button">
              Find a deal
            </a>
          </div>
          <div className="home-coverage" aria-label="Catalog coverage">
            <span>
              <strong>{money(data.coverage.products)}</strong> listings
            </span>
            <i />
            <span>
              <strong>{data.coverage.retailers}</strong> retailers
            </span>
            <i />
            <span>Free to use</span>
          </div>
          {updated && (
            <p className="home-freshness">
              {data.coverage.last_scan_at ? 'Last completed scan' : 'Latest catalog change'} ·{' '}
              <time dateTime={updated}>
                {new Date(updated).toLocaleString('en-GB', {
                  day: 'numeric',
                  month: 'short',
                  hour: '2-digit',
                  minute: '2-digit',
                })}
              </time>
            </p>
          )}
        </div>
        {featured ? (
          <a
            className="hero-product"
            href={`/product/${encodeURIComponent(featured.id)}`}
            onClick={go}
          >
            <div className="hero-product-label">
              <ShieldCheck size={14} /> Featured offer
            </div>
            <ProductImage product={featured} />
            <div className="hero-product-copy">
              <span>
                {sectionLabel(featured.section)} · {featured.site}
              </span>
              <h2>{featured.title}</h2>
              <div>
                <strong>
                  {money(featured.price)} <small>DT</small>
                </strong>
                {featuredDiscount && <del>{money(featuredDiscount.original)} DT</del>}
              </div>
              {featuredDiscount && (
                <p className="hero-price-context">
                  Save {money(featuredDiscount.saving)} DT · {featuredDiscount.percent || '<1'}% off
                </p>
              )}
              <b>
                See the price history <ArrowRight size={15} />
              </b>
            </div>
          </a>
        ) : (
          <div className="hero-featured-empty">
            <Bell size={32} />
            <h2>A better price is worth watching.</h2>
            <p>Save an offer and get an email when it matches your price target.</p>
            <a href="/alerts" className="secondary-button" onClick={go}>
              Explore price alerts <ArrowRight size={16} />
            </a>
          </div>
        )}
      </section>
      {error && (
        <div className="home-inline-error" role="alert">
          <span>Showing the last loaded catalog. {error}</span>
          <button onClick={() => setRevision((value) => value + 1)}>Try again</button>
        </div>
      )}
      <section
        className="home-explore home-shortcuts"
        id="categories"
        aria-labelledby="category-title"
      >
        <div className="home-section-heading">
          <h2 id="category-title">What’s your next upgrade?</h2>
          <button
            className="home-text-button"
            aria-expanded={allCategories}
            aria-controls="home-all-categories"
            onClick={() => setAllCategories((value) => !value)}
          >
            {allCategories ? 'Fewer categories' : 'All categories'} <ArrowRight size={15} />
          </button>
        </div>
        <div className="home-shortcut-grid">
          {primaryCategories.map((id) => {
            const Icon = categoryIcons[id as keyof typeof categoryIcons] || PackageSearch;
            return (
              <a href={`/${id}`} key={id} onClick={go}>
                <Icon size={24} strokeWidth={1.5} />
                <span>{sectionLabel(id)}</span>
              </a>
            );
          })}
        </div>
        {allCategories && (
          <div className="home-all-categories" id="home-all-categories">
            {Object.entries(categories).map(([group, entries]) => (
              <div key={group}>
                <h3>{group}</h3>
                {entries.map((item) => (
                  <a href={`/${item.id}`} key={item.id} onClick={go}>
                    {item.name}
                  </a>
                ))}
              </div>
            ))}
          </div>
        )}
      </section>
      <section
        className="home-section home-deals-section"
        id="deals"
        aria-labelledby="deals-heading"
      >
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">Browse your next upgrade</span>
            <h2 id="deals-heading">Featured categories</h2>
            <p>Discounted offers from Tunisian retailers.</p>
          </div>
        </div>
        {data.featured_sections.map((section) => {
          const products = data.deals
            .filter((product) => product.section === section && retailerDiscount(product))
            .slice(0, 12);
          return (
            <section
              className="home-featured-category"
              key={section}
              aria-labelledby={`featured-${section}`}
            >
              <div className="home-section-heading">
                <h3 id={`featured-${section}`}>{sectionLabel(section)}</h3>
                <a className="home-text-button" href={`/${section}`} onClick={go}>
                  View category <ArrowRight size={15} />
                </a>
              </div>
              {products.length > 0 && (
                <CategoryPreviews section={section} products={products} onNavigate={onNavigate} />
              )}
              {!products.length && (
                <p className="home-category-empty">
                  No discounted offers right now. Browse the category for current listings.
                </p>
              )}
            </section>
          );
        })}
        <p className="discovery-footnote">
          Discounts compare the retailer’s original and current prices. Confirm the offer with the
          retailer.
        </p>
      </section>
      <section className="home-alert-banner" aria-labelledby="home-alert-title">
        <div className="home-alert-icon">
          <Bell size={25} />
        </div>
        <div>
          <span className="home-kicker">Less checking. Better timing.</span>
          <h2 id="home-alert-title">Let your next upgrade come to you.</h2>
          <p>
            Choose a product, set your target, and get an email when an offer matches. Price alerts
            are free.
          </p>
        </div>
        <a href="/alerts" className="primary-button" onClick={go}>
          Create a price alert <ArrowRight size={16} />
        </a>
      </section>
      <section className="home-market home-market-compact" aria-labelledby="market-heading">
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">A little perspective</span>
            <h2 id="market-heading">Where are prices heading?</h2>
          </div>
          <a className="home-text-button" href={marketHref} onClick={go}>
            Explore market insights <ArrowRight size={16} />
          </a>
        </div>
        <div className="home-snapshot-layout">
          <div className="home-snapshot-chart">
            <div className="market-index-row">
              <div>
                <span>
                  {market.section ? sectionLabel(market.section) : 'Grouped categories'} ·
                  comparable prices
                </span>
                <strong
                  className={
                    market.change_percent != null && market.change_percent > 0
                      ? 'home-price-up'
                      : market.change_percent != null && market.change_percent < 0
                        ? 'home-price-down'
                        : undefined
                  }
                >
                  {market.change_percent == null
                    ? 'Collecting data'
                    : signedPercent(market.change_percent)}
                </strong>
              </div>
              <span className="market-period-label">30-day view</span>
            </div>
            {market.points.length > 1 ? (
              <MarketTrendChart
                title="Change in comparable recorded prices"
                points={market.points.map((point) => ({
                  timestamp: point.timestamp,
                  value: point.index - 100,
                  sampleSize: point.comparable_products,
                }))}
                valueLabel={signedPercent}
                tickLabel={signedPercent}
                baseline={0}
                colorBySign
                minimumPadding={0.5}
              />
            ) : (
              <div className="market-chart-empty">
                <TrendingUp />
                <strong>Building a comparable trend</strong>
                <span>At least five matching offers across two observations are needed.</span>
              </div>
            )}
            <p className="discovery-footnote">
              {historyLabel(market.points)} · {money(market.comparable_products)} offers in the
              latest comparison
            </p>
          </div>
          <div className="home-category-trends">
            <span className="home-kicker">Choose a market</span>
            {data.market_views
              .filter((view) => view.range === '30d')
              .map((view) => (
                <button
                  key={view.section || 'All'}
                  aria-pressed={marketSection === (view.section || 'All')}
                  onClick={() => setMarketSection(view.section || 'All')}
                >
                  <span>{view.section ? sectionLabel(view.section) : 'Grouped categories'}</span>
                  <strong
                    className={
                      view.change_percent != null && view.change_percent > 0
                        ? 'home-price-up'
                        : view.change_percent != null && view.change_percent < 0
                          ? 'home-price-down'
                          : ''
                    }
                  >
                    {view.change_percent == null ? '—' : signedPercent(view.change_percent)}
                  </strong>
                </button>
              ))}
            <p>
              Changes cover available history within the selected period. Only categories with
              product grouping are included.
            </p>
          </div>
        </div>
      </section>
      <section className="home-section home-arrivals" aria-labelledby="arrival-title">
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">Fresh discoveries</span>
            <h2 id="arrival-title">New on Flagship</h2>
          </div>
          <a className="home-text-button" href="/activity" onClick={go}>
            Catalog activity <ArrowRight size={15} />
          </a>
        </div>
        <div className="home-arrival-grid">
          {data.new_products.slice(0, 6).map((product) => (
            <a key={product.id} href={`/product/${encodeURIComponent(product.id)}`} onClick={go}>
              <ProductImage product={product} />
              <div>
                <span className="home-product-section">{sectionLabel(product.section)}</span>
                <h3>{product.title}</h3>
                <strong>{money(product.price)} DT</strong>
                <small>{product.site}</small>
                <time
                  className="home-added-time"
                  dateTime={product.added_at}
                  title={new Date(product.added_at).toLocaleString('en-GB')}
                >
                  <Clock3 size={12} />
                  {addedAgo(product.added_at, now)}
                </time>
              </div>
              <ArrowRight size={15} />
            </a>
          ))}
        </div>
      </section>
      <section className="home-retailer-coverage" aria-labelledby="retailer-coverage-title">
        <div className="home-section-heading">
          <div>
            <span className="home-kicker">
              <Store size={14} /> Retailer coverage
            </span>
            <h2 id="retailer-coverage-title">
              Across {data.coverage.retailers} Tunisian retailers
            </h2>
            <p>Compare here. Buy directly from your chosen retailer.</p>
          </div>
          {data.retailers.length > 8 && (
            <button
              className="home-text-button"
              aria-expanded={allRetailers}
              aria-controls="home-retailer-list"
              onClick={() => setAllRetailers((value) => !value)}
            >
              {allRetailers ? 'Show fewer' : `Show all ${data.retailers.length}`}{' '}
              <ArrowRight size={15} />
            </button>
          )}
        </div>
        <div className="home-coverage-grid" id="home-retailer-list">
          {(allRetailers ? data.retailers : data.retailers.slice(0, 8)).map((retailer) => (
            <div className="home-coverage-retailer" key={retailer.name}>
              <span aria-hidden="true">{retailer.name.slice(0, 2).toUpperCase()}</span>
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
    </main>
  );
}

function CategoryPreviews({
  section,
  products,
  onNavigate,
}: {
  section: string;
  products: HomeProduct[];
  onNavigate: (path: string) => void;
}) {
  const track = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ start: 0, visible: 4 });
  const updatePosition = () => {
    const element = track.current;
    const first = element?.firstElementChild;
    if (!element || !first) return;
    const gap = parseFloat(getComputedStyle(element).columnGap) || 0;
    const step = first.getBoundingClientRect().width + gap;
    const visible = Math.max(1, Math.round((element.clientWidth + gap) / step));
    const start = Math.min(
      Math.max(0, products.length - visible),
      Math.max(0, Math.round(element.scrollLeft / step)),
    );
    setPosition((previous) =>
      previous.start === start && previous.visible === visible ? previous : { start, visible },
    );
  };
  useEffect(() => {
    const element = track.current;
    if (!element) return;
    const observer = new ResizeObserver(updatePosition);
    observer.observe(element);
    updatePosition();
    return () => observer.disconnect();
  }, [products.length]);
  const move = (direction: number) => {
    const element = track.current;
    if (!element) return;
    const gap = parseFloat(getComputedStyle(element).columnGap) || 0;
    element.scrollBy({
      left: direction * (element.clientWidth + gap),
      behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches
        ? 'instant'
        : 'smooth',
    });
  };
  return (
    <div className="home-category-carousel">
      <div className="home-category-slider">
        <button
          type="button"
          className="home-category-arrow previous"
          aria-label={`Previous ${sectionLabel(section)} offers`}
          aria-controls={`offers-${section}`}
          disabled={position.start === 0}
          onClick={() => move(-1)}
        >
          <ChevronLeft size={20} />
        </button>
        <div
          className="home-category-products"
          id={`offers-${section}`}
          ref={track}
          onScroll={updatePosition}
          tabIndex={0}
          aria-label={`${sectionLabel(section)} discounted offers`}
          onKeyDown={(event) => {
            if (event.target !== event.currentTarget) return;
            if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
              event.preventDefault();
              move(event.key === 'ArrowLeft' ? -1 : 1);
            }
          }}
        >
          {products.map((product) => (
            <DiscountPreview key={product.id} product={product} onNavigate={onNavigate} />
          ))}
        </div>
        <button
          type="button"
          className="home-category-arrow next"
          aria-label={`Next ${sectionLabel(section)} offers`}
          aria-controls={`offers-${section}`}
          disabled={position.start + position.visible >= products.length}
          onClick={() => move(1)}
        >
          <ChevronRight size={20} />
        </button>
      </div>
      <p className="home-category-position">
        {position.start + 1}
        {position.visible > 1 && `–${Math.min(products.length, position.start + position.visible)}`}
        <span> of {products.length} offers</span>
      </p>
    </div>
  );
}

function DiscountPreview({
  product,
  onNavigate,
}: {
  product: HomeProduct;
  onNavigate: (path: string) => void;
}) {
  const discount = retailerDiscount(product);
  return (
    <a
      className="deal-card home-discount-card"
      href={`/product/${encodeURIComponent(product.id)}`}
      onClick={(event) => {
        if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        onNavigate(event.currentTarget.pathname);
      }}
    >
      <div className="home-discount-image">
        <ProductImage product={product} />
        {discount && (
          <span className="home-discount-badge">
            {discount.percent ? `−${discount.percent}%` : '<1% off'}
          </span>
        )}
      </div>
      <h3>{product.title}</h3>
      <div className="deal-price">
        <strong>
          {money(product.price)} <small>DT</small>
        </strong>
        {discount && (
          <div className="home-discount-reference">
            <del>{money(discount.original)} DT</del>
            <span className="home-discount-saving">Save {money(discount.saving)} DT</span>
          </div>
        )}
      </div>
      <span className="deal-retailer">
        {product.site}
        <ArrowRight size={14} />
      </span>
    </a>
  );
}
