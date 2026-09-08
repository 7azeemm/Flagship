import { useState } from 'react';
import {
  Activity,
  Star,
  ArrowUpRight,
  CalendarDays,
  Clock3,
  History,
  Maximize2,
  Package,
} from 'lucide-react';
import type { Product, VariantChart } from '../types';
import { fieldLabel, money, safeURL, sectionLabel, stockLabels } from '../lib/catalog';
import { ProductAnalytics } from './ProductAnalytics';
import { Dialog } from './Dialog';
import { Price, ProductImage, Specifications } from './ProductCard';

const formatDate = (value: string | null) => {
  if (!value) return 'Not available';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return 'Not available';
  return date.toLocaleString('en-GB', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
};

export function ProductDialog({
  products,
  chart,
  fields,
  endpoint,
  onClose,
}: {
  products: Product[];
  chart: VariantChart | null;
  fields: string[];
  endpoint: string;
  onClose: () => void;
}) {
  const offers = [...products].sort(
    (a, b) => Number(b.status === 'InStock') - Number(a.status === 'InStock') || a.price - b.price,
  );
  const [selected, setSelected] = useState(offers[0]);
  const [zoom, setZoom] = useState(false);
  const link = safeURL(selected.url);

  return (
    <Dialog title={selected.title} onClose={onClose} className="product-dialog">
      <div className="product-detail-grid">
        <section className="detail-overview" aria-labelledby="product-overview-title">
          <button
            className={`detail-image ${zoom ? 'zoomed' : ''}`}
            onClick={() => setZoom(!zoom)}
            aria-label={zoom ? 'Reduce product image' : 'Enlarge product image'}
            aria-pressed={zoom}
          >
            <ProductImage key={selected.image} product={selected} eager />
            <Maximize2 size={18} />
          </button>
          <div className="detail-meta">
            <span className="retailer-badge">{selected.site}</span>
            <span className={`stock stock-${selected.status}`}>
              {stockLabels[selected.status] || 'Availability unknown'}
            </span>
          </div>
          <h2 id="product-overview-title" tabIndex={-1}>{selected.title}</h2>
          <Price product={selected} />
          <div className="detail-description">
            <h3>Description</h3>
            <p>{selected.description || 'No description is available for this product yet.'}</p>
          </div>
          {link ? (
            <a
              className="primary-button visit-retailer"
              href={link}
              target="_blank"
              rel="noopener noreferrer"
            >
              Visit {selected.site} <ArrowUpRight size={17} />
            </a>
          ) : (
            <p className="muted">Retailer link unavailable.</p>
          )}
        </section>

        <section className="detail-information" aria-label="Product specifications and information">
          <div className="detail-panel">
            <div className="detail-panel-heading">
              <Package size={17} />
              <div>
                <h2>Components</h2>
                <p>Configuration reported by the retailer</p>
              </div>
            </div>
            <Specifications product={selected} fields={fields} />
          </div>

          <div className="detail-panel product-record">
            <div className="detail-panel-heading">
              <Clock3 size={17} />
              <div>
                <h2>Product information</h2>
                <p>Catalog record and update activity</p>
              </div>
            </div>
            <dl className="product-facts">
              <div>
                <dt>Retailer</dt>
                <dd>{selected.site}</dd>
              </div>
              <div>
                <dt>Category</dt>
                <dd>{sectionLabel(selected.section)}</dd>
              </div>
              <div>
                <dt>
                  <CalendarDays size={13} /> Added
                </dt>
                <dd>{formatDate(selected.added_at)}</dd>
              </div>
              <div>
                <dt>
                  <Clock3 size={13} /> Last updated
                </dt>
                <dd>{formatDate(selected.updated_at)}</dd>
              </div>
              <div>
                <dt>
                  <History size={13} /> History
                </dt>
                <dd>
                  {selected.history.length
                    ? `${selected.history.length} recorded changes`
                    : 'No recorded changes'}
                </dd>
              </div>
              <div>
                <dt>Product ID</dt>
                <dd className="product-id">{selected.id}</dd>
              </div>
            </dl>
          </div>
        <div className="future-insights">
          <section>
            <Activity size={18} />
            <div>
              <h3>Performance</h3>
              <p>Benchmarks and comparisons coming later.</p>
            </div>
            <span>Planned</span>
          </section>
          <section>
            <Star size={18} />
            <div>
              <h3>Ratings</h3>
              <p>Reviews and rating breakdowns coming later.</p>
            </div>
            <span>Planned</span>
          </section>
        </div>
        </section>
        {offers.length > 0 && (
          <section className="offer-list detail-panel detail-listings">
            <h2>{offers.length} retailer {offers.length === 1 ? 'listing' : 'listings'}</h2>
            <p>Check each retailer’s configuration before choosing.</p>
            {offers.map((offer) => (
              <button
                className={selected.id === offer.id ? 'active' : ''}
                key={offer.id}
                onClick={(event) => {
                  const dialog = event.currentTarget.closest('dialog');
                  setSelected(offer);
                  setZoom(false);
                  requestAnimationFrame(() => {
                    dialog?.scrollTo({ top: 0, behavior: 'instant' });
                    dialog?.querySelector<HTMLElement>('#product-overview-title')?.focus({ preventScroll: true });
                  });
                }}
                aria-pressed={selected.id === offer.id}
              >
                <span>
                  {offer.site}
                  <small>{stockLabels[offer.status] || 'Availability unknown'}</small>
                </span>
                <strong>{money(offer.price)} DT</strong>
                <ArrowUpRight size={16} />
              </button>
            ))}
          </section>
        )}
        <ProductChangeHistory key={selected.id} product={selected} />
        <ProductAnalytics initialChart={chart} endpoint={endpoint} product={selected} />
      </div>
    </Dialog>
  );
}


function ProductChangeHistory({ product }: { product: Product }) {
  const events = product.history
    .filter((event): event is Record<string, unknown> => !!event && typeof event === 'object' && !Array.isArray(event))
    .filter((event) => typeof event.field === 'string')
    .sort((a, b) => (Date.parse(String(b.timestamp)) || 0) - (Date.parse(String(a.timestamp)) || 0));
  const value = (field: string, raw: unknown): string => {
    if (raw === null || raw === undefined || raw === '') return 'Not recorded';
    if ((field === 'price' || field === 'original_price') && typeof raw === 'number') return `${money(raw)} DT`;
    if (field === 'status' && typeof raw === 'string') return stockLabels[raw] || raw;
    if (typeof raw === 'object') return JSON.stringify(raw);
    return String(raw);
  };
  return (
    <section className="detail-panel product-change-history" aria-label="Product history">
      <div className="detail-panel-heading">
        <History size={17} />
        <div><h2>Product history</h2><p>Recorded changes for the selected {product.site} listing · newest first</p></div>
      </div>
      {events.length ? (
        <ol className="product-timeline">
          {events.map((event, index) => (
            <li key={index}>
              <div><strong>{fieldLabel(String(event.field))}</strong><time>{formatDate(typeof event.timestamp === 'string' ? event.timestamp : null)}</time></div>
              <p><span>{value(String(event.field), event.old_value)}</span><span aria-label="changed to"> → </span><strong>{value(String(event.field), event.new_value)}</strong></p>
            </li>
          ))}
        </ol>
      ) : <p className="muted">No changes recorded yet. Price, availability, and other product updates will appear here.</p>}
    </section>
  );
}
