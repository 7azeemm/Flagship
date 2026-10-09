import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { ArrowRight, ArrowUp, Info } from 'lucide-react';
import type { StatisticsData } from '../types';
import { money } from '../lib/catalog';
import { minimumRetailerComparisons, retailerLandscape } from '../lib/insights';
import { retailerLabelLayout } from '../lib/retailerLabels';
import { zeroBasedAxis } from '../lib/chartScale';
import { ChartTooltip, type ChartCursor } from './ChartTooltip';

export function RetailerLandscape({
  overview,
  section,
  range,
  onNavigate,
}: {
  overview: StatisticsData['retailer_overviews'][string] | undefined;
  section: string;
  range: string;
  onNavigate: (path: string) => void;
}) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(920);
  const [hovered, setHovered] = useState('');
  const [cursor, setCursor] = useState<ChartCursor | null>(null);
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const resize = () => setWidth(Math.max(280, element.getBoundingClientRect().width));
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const stores = useMemo(() => retailerLandscape(overview?.retailers || []), [overview]);
  const activeName = hovered;
  const active = stores.find((store) => store.name === activeName);
  const left = 58;
  const right = width - 24;
  const top = 40;
  const priceAxis = zeroBasedAxis(
    stores.map((store) => store.value),
    { maximumValue: 100 },
  );
  const stockAxis = zeroBasedAxis(
    stores.map((store) => store.inStock),
    { integerTicks: true },
  );
  const { bottom, height, labels } = useMemo(
    () => retailerLabelLayout(stores, width, stockAxis.maximum, priceAxis.maximum),
    [stores, width, stockAxis.maximum, priceAxis.maximum],
  );
  const x = (value: number) => left + (value / priceAxis.maximum) * (right - left);
  const y = (value: number) => bottom - (value / stockAxis.maximum) * (bottom - top);
  const pathFor = (name: string) =>
    `/statistics?${new URLSearchParams({ view: 'explorer', section, range, retailer: name })}`;
  const follow = (event: React.MouseEvent<SVGAElement | HTMLAnchorElement>, name: string) => {
    if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    onNavigate(pathFor(name));
  };
  const plotted = active
    ? [...stores.filter((store) => store.name !== activeName), active]
    : stores;
  const ready = overview?.retailers.some(
    (store) => store.in_stock_offers !== undefined && store.within_two_percent !== undefined,
  );
  return (
    <section className="retailer-landscape" aria-labelledby={`${id}-heading`}>
      <div className="insight-section-heading">
        <div>
          <h2 id={`${id}-heading`}>Price ranking vs in-stock selection</h2>
          <p>Find stores that combine competitive prices with more products in stock.</p>
        </div>
      </div>
      {stores.length > 0 ? (
        <>
          <div className="retailer-landscape-guide">
            <span>
              <ArrowRight size={14} /> More competitive prices
            </span>
            <span>
              <ArrowUp size={14} /> More products in stock
            </span>
            <small>
              {stores.length} retailers · {minimumRetailerComparisons}+ price comparisons · current
              snapshot
            </small>
          </div>
          <div className="retailer-landscape-chart" ref={root}>
            <svg
              viewBox={`0 0 ${width} ${height}`}
              style={{ height }}
              role="group"
              aria-labelledby={`${id}-title ${id}-description`}
              onPointerLeave={() => {
                setHovered('');
                setCursor(null);
              }}
            >
              <title id={`${id}-title`}>Retailer prices and available selection</title>
              <desc id={`${id}-description`}>
                Each dot is a retailer. Right means a stronger price score, adjusted for sample
                size. Up means more individual products in stock. Focus a dot for exact figures.
                Activate a dot to explore its offers. The green upper-right area indicates stronger
                prices and a wider in-stock selection within this category.
              </desc>
              <defs>
                <linearGradient id={`${id}-opportunity`} x1="0%" y1="100%" x2="100%" y2="0%">
                  <stop offset="0%" className="retailer-landscape-green-stop" stopOpacity="0" />
                  <stop
                    offset="100%"
                    className="retailer-landscape-green-stop"
                    stopOpacity="0.16"
                  />
                </linearGradient>
              </defs>
              <rect
                x={x(priceAxis.maximum / 2)}
                y={top}
                width={right - x(priceAxis.maximum / 2)}
                height={y(stockAxis.maximum / 2) - top}
                fill={`url(#${id}-opportunity)`}
                className="retailer-landscape-opportunity"
                aria-hidden="true"
              />
              <text x={left} y="17" className="retailer-landscape-axis-title">
                Products in stock
              </text>
              {stockAxis.ticks.map((value) => (
                <g key={value} className="retailer-landscape-y-tick">
                  <line
                    x1={left}
                    x2={right}
                    y1={y(value)}
                    y2={y(value)}
                    className="retailer-landscape-grid"
                  />
                  <text
                    x={left - 12}
                    y={y(value) + 4}
                    textAnchor="end"
                    className="retailer-landscape-tick"
                  >
                    {money(value)}
                  </text>
                </g>
              ))}
              {priceAxis.ticks.map((value) => (
                <g key={value} className="retailer-landscape-x-tick">
                  <line
                    x1={x(value)}
                    x2={x(value)}
                    y1={top}
                    y2={bottom}
                    className="retailer-landscape-grid is-vertical"
                  />
                  <text
                    x={x(value)}
                    y={bottom + 23}
                    textAnchor="middle"
                    className="retailer-landscape-tick"
                  >
                    {value}
                  </text>
                </g>
              ))}
              <text
                x={(left + right) / 2}
                y={height - 8}
                textAnchor="middle"
                className="retailer-landscape-axis-title"
              >
                Price score / 100
              </text>
              {plotted.map((store) => {
                const highlighted = store.name === activeName;
                return (
                  <a
                    key={store.name}
                    className={`retailer-landscape-dot${highlighted ? ' is-active' : ''}${activeName && !highlighted ? ' is-muted' : ''}`}
                    href={pathFor(store.name)}
                    aria-label={`${store.name}: ${store.value.toFixed(1)} price score out of 100, ${store.competitiveShare.toFixed(1)}% within 2%, ${money(store.inStock)} products in stock, ${money(store.offers)} total offers, ${money(store.price_comparisons)} price comparisons. Explore offers.`}
                    onClick={(event) => follow(event, store.name)}
                    onPointerMove={(event) => {
                      setHovered(store.name);
                      setCursor({ x: event.clientX, y: event.clientY });
                    }}
                    onPointerLeave={() => {
                      setHovered('');
                      setCursor(null);
                    }}
                    onFocus={(event) => {
                      const rect = event.currentTarget.getBoundingClientRect();
                      setHovered(store.name);
                      setCursor({ x: rect.left + rect.width / 2, y: rect.top });
                    }}
                    onBlur={() => {
                      setHovered('');
                      setCursor(null);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'Escape') {
                        setHovered('');
                        setCursor(null);
                      }
                    }}
                  >
                    <circle
                      cx={x(store.value)}
                      cy={y(store.inStock)}
                      r="16"
                      className="retailer-landscape-hit"
                    />
                    <circle
                      cx={x(store.value)}
                      cy={y(store.inStock)}
                      r={highlighted ? 11 : 9}
                      className="retailer-landscape-halo"
                    />
                    <circle
                      cx={x(store.value)}
                      cy={y(store.inStock)}
                      r={highlighted ? 6.5 : 5}
                      className="retailer-landscape-point"
                    />
                  </a>
                );
              })}
              {labels.map((label) => (
                <g key={label.name} className="retailer-landscape-label">
                  <text x={label.x + 6} y={label.y + 16}>
                    {label.name}
                  </text>
                </g>
              ))}
            </svg>
          </div>
          <p className="retailer-landscape-caption">
            <span className="retailer-landscape-green-key">
              <i aria-hidden="true" /> Upper right: stronger prices + more stock
            </span>
            <span>
              Hover or focus a dot for details. Select a dot to explore that retailer’s offers.
            </span>
          </p>
        </>
      ) : (
        <div className="statistics-empty">
          <strong>
            {ready
              ? 'Not enough shared price comparisons yet.'
              : 'Retailer comparison data is not available yet.'}
          </strong>
          <span>
            Choose another category to compare retailers with {minimumRetailerComparisons} or more
            shared configurations.
          </span>
        </div>
      )}
      <details className="insight-method">
        <summary>
          <Info size={14} /> How to read this chart
        </summary>
        <p>
          Further right means a stronger price score, using the same sample adjustment as the price
          ranking above. Higher means it has more individual in-stock listings in the selected
          category. Each dot has the same size.
        </p>
        <p>
          Both axes scale to the retailers in this view. Scores remain out of 100. The green
          upper-right area highlights stronger prices and a wider in-stock selection relative to the
          selected category.
        </p>
        <p>
          Retailers need at least {minimumRetailerComparisons} shared price comparisons to appear.
          Stock counts include all tracked in-stock listings, including multiple listings with the
          same specifications. Coverage can include more retailers: this chart also needs enough
          shared configurations for a price score. Price comparisons cover configurations shared
          with another retailer, so product mixes and sample sizes differ. This is a current
          snapshot of prices and selection.
        </p>
      </details>
      <ChartTooltip cursor={cursor}>
        {active && (
          <>
            <strong>{active.name}</strong>
            <span>Price score: {active.value.toFixed(1)} / 100</span>
            <span>{active.competitiveShare.toFixed(1)}% cheapest or within 2%</span>
            <span>{money(active.inStock)} products in stock</span>
            <span>{money(active.offers)} total offers across all statuses</span>
            <span>{money(active.price_comparisons)} shared price comparisons</span>
            <small>Select to explore offers</small>
          </>
        )}
      </ChartTooltip>
    </section>
  );
}
