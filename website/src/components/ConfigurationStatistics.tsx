import { BarChart3, Info } from 'lucide-react';
import type { StatisticsData } from '../types';
import { money, sectionLabel } from '../lib/catalog';

export function ConfigurationStatistics({
  data,
  section,
  lowestPrice,
}: {
  data: StatisticsData['offer_summary'];
  section: string;
  lowestPrice: number | null;
}) {
  if (!data)
    return (
      <section className="statistics-empty" aria-label="Matching offer statistics">
        <BarChart3 size={26} />
        <strong>Offer summaries are waiting for the updated backend.</strong>
      </section>
    );
  const availability = data.offer_count ? (100 * data.available_offers) / data.offer_count : 0;
  const largestBand = Math.max(1, ...data.price_bands.map((band) => band.count));
  return (
    <section className="configuration-statistics" aria-labelledby="configuration-statistics-title">
      <div className="insight-section-heading">
        <div>
          <h2 id="configuration-statistics-title">Matching offer analysis</h2>
          <p>
            {sectionLabel(section)} · {money(data.offer_count)} matching offers across{' '}
            {data.retailer_count} {data.retailer_count === 1 ? 'retailer' : 'retailers'} · current
            snapshot
          </p>
        </div>
      </div>
      <div className="configuration-summary-cards">
        <article>
          <span>Current lowest price</span>
          <strong>
            {lowestPrice === null ? (
              '—'
            ) : (
              <>
                {money(lowestPrice)} <small className="configuration-price-unit">DT</small>
              </>
            )}
          </strong>
          <small>Cheapest matching in-stock offer</small>
        </article>
        <article>
          <span>Average price</span>
          <strong>
            {data.average_price === null ? (
              '—'
            ) : (
              <>
                {money(data.average_price)} <small className="configuration-price-unit">DT</small>
              </>
            )}
          </strong>
          <small>Across {money(data.priced_offers)} priced offers · all statuses</small>
        </article>
        <article>
          <span>All products</span>
          <strong>{money(data.offer_count)}</strong>
          <small>Every matching listing · all statuses</small>
        </article>
        <article>
          <span>In-stock products</span>
          <strong>{money(data.available_offers)}</strong>
          <small>
            {data.offer_count
              ? `${availability.toFixed(1)}% of matching offers are in stock`
              : 'No matching listings'}
          </small>
        </article>
      </div>
      {!data.offer_count ? (
        <div className="statistics-card statistics-empty">
          <BarChart3 size={26} />
          <strong>No matching offers</strong>
          <span>Clear refinements or choose another category or retailer.</span>
        </div>
      ) : (
        <div className="configuration-visuals">
          <article className="statistics-card configuration-availability-card">
            <h3>In-stock rate</h3>
            <p>How many matching listings are in stock right now?</p>
            <div className="configuration-availability-visual">
              <div
                className="configuration-availability-ring"
                role="img"
                aria-label={`${availability.toFixed(1)}% of matching offers are in stock`}
                style={{
                  background: `conic-gradient(var(--price-down) ${availability}%, var(--border) 0)`,
                }}
              >
                <span>
                  <strong>{availability.toFixed(1)}%</strong>
                  <small>in stock</small>
                </span>
              </div>
              <div className="configuration-availability-key">
                <span>
                  <i className="available" />
                  In stock <strong>{money(data.available_offers)}</strong>
                </span>
                <span>
                  <i />
                  Other statuses <strong>{money(data.offer_count - data.available_offers)}</strong>
                </span>
              </div>
            </div>
            <p className="configuration-chart-note">
              Each retailer listing counts separately, including offers for the same product.
            </p>
          </article>
          <article className="statistics-card configuration-price-distribution">
            <h3>In-stock prices</h3>
            <p>All matching in-stock offers, distributed across price ranges.</p>
            {data.available_priced_offers ? (
              <figure aria-label="Price distribution of matching in-stock offers">
                <div className="configuration-price-bands">
                  {data.price_bands.map((band) => (
                    <div key={band.from} className="configuration-price-band">
                      <div className="configuration-price-column">
                        <strong>{money(band.count)}</strong>
                        <span style={{ height: `${(84 * band.count) / largestBand}%` }} />
                      </div>
                      <span className="configuration-price-range">
                        {band.to === null
                          ? `${money(band.from)}+`
                          : `${money(band.from)}–${money(band.to)}`}
                      </span>
                    </div>
                  ))}
                </div>
                <figcaption>
                  Price ranges in DT · {money(data.available_priced_offers)} in-stock offers with a
                  listed price
                </figcaption>
              </figure>
            ) : (
              <div className="statistics-empty">
                <strong>No matching in-stock offers with a listed price</strong>
              </div>
            )}
            <p className="configuration-chart-note">
              Each positive in-stock price contributes once. Range upper bounds belong to the next
              band.
            </p>
          </article>
        </div>
      )}
      <details className="configuration-method">
        <summary>
          <Info size={14} /> How these offers are counted
        </summary>
        <p>
          Every current listing that matches the category, retailer and specifications counts once.
          In-stock counts use the stock status, even when the price is missing. On-demand, arriving
          and out-of-stock listings count under other statuses. Average price is the arithmetic mean
          of positive listed prices across all statuses. The price ranges include every positive
          in-stock price.
        </p>
      </details>
    </section>
  );
}
