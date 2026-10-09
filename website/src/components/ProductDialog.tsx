import { useEffect, useMemo, useState } from 'react';
import {
  ArrowUpRight,
  CalendarDays,
  Clock3,
  History,
  Gauge,
  Star,
  Maximize2,
  Package,
  Store,
  Tags,
} from 'lucide-react';
import type { Product, VariantChart } from '../types';
import {
  componentLabel,
  fieldLabel,
  money,
  safeURL,
  sectionLabel,
  stockLabels,
} from '../lib/catalog';
import { ProductAnalytics } from './ProductAnalytics';
import { Dialog } from './Dialog';
import { ProductAlertButton } from './AlertsPage';
import { ShareButton } from './ShareButton';
import {
  Price,
  ProductGallery,
  Specifications,
  WishlistButton,
  sortComponentFields,
} from './ProductCard';

const formatDate = (value: string | null) => {
  if (!value) return 'Not available';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return 'Not available';
  const absolute = date.toLocaleString('en-GB', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
  const elapsed = Math.max(0, Math.floor((Date.now() - date.getTime()) / 1000));
  const relative =
    elapsed < 60
      ? 'Just now'
      : new Intl.RelativeTimeFormat('en', { numeric: 'always' }).format(
          -Math.floor(elapsed / (elapsed < 3600 ? 60 : elapsed < 86400 ? 3600 : 86400)),
          elapsed < 3600 ? 'minute' : elapsed < 86400 ? 'hour' : 'day',
        );
  return absolute + ' · ' + relative;
};

export function ProductDialog({
  offers,
  chart,
  fields,
  detailsEndpoint,
  initialProductId,
  onClose,
  onSelectProduct,
  embedded = false,
  computerSections = [],
}: {
  offers: Product[];
  deals?: Record<string, import('../types').DealEvidence>;
  chart: VariantChart | null;
  fields: string[];
  detailsEndpoint: string;
  initialProductId?: string;
  onClose: () => void;
  onSelectProduct?: (product: Product) => void;
  embedded?: boolean;
  computerSections?: string[];
}) {
  const sortedOffers = useMemo(
    () =>
      [...offers].sort(
        (a, b) =>
          Number(b.status === 'InStock') - Number(a.status === 'InStock') || a.price - b.price,
      ),
    [offers],
  );
  const [selected, setSelected] = useState(
    sortedOffers.find((offer) => offer.id === initialProductId) || sortedOffers[0],
  );
  const [zoom, setZoom] = useState(false);
  useEffect(() => {
    setSelected(sortedOffers.find((offer) => offer.id === initialProductId) || sortedOffers[0]);
    setZoom(false);
  }, [initialProductId, sortedOffers]);
  const link = safeURL(selected.url);
  const componentFields = sortComponentFields(
    [...new Set([...fields, ...Object.keys(selected.components)])].filter((field) =>
      componentLabel(selected.components[field]).trim(),
    ),
  );
  useEffect(() => {
    document.title = `${selected.title} — Flagship`;
    onSelectProduct?.(selected);
  }, [selected, onSelectProduct]);

  const content = (
    <div className="product-detail-grid">
      <section className="detail-overview" aria-labelledby="product-overview-title">
        <ProductGallery
          key={`gallery-${selected.id}`}
          product={selected}
          className={`detail-image ${zoom ? 'zoomed' : ''}`}
          onOpen={() => setZoom(!zoom)}
          label={zoom ? 'Reduce product image' : 'Enlarge product image'}
          pressed={zoom}
          eager
        >
          <Maximize2 size={18} />
        </ProductGallery>
        <div className="detail-meta">
          <span className="retailer-badge">{selected.site}</span>
          <span className={`stock stock-${selected.removed_at ? 'OutOfStock' : selected.status}`}>
            {selected.removed_at
              ? 'Listing removed'
              : stockLabels[selected.status] || 'Availability unknown'}
          </span>
          {!selected.removed_at && (
            <WishlistButton
              targetType={detailsEndpoint.startsWith('/variants/') ? 'variant' : 'product'}
              targetId={decodeURIComponent(detailsEndpoint.split('/').pop() || selected.id)}
              name={selected.title}
            />
          )}
        </div>
        <h2 id="product-overview-title" tabIndex={-1}>
          {selected.title}
        </h2>
        {selected.removed_at && (
          <p className="muted">
            Removed {formatDate(selected.removed_at)}. Details and price below reflect the last
            recorded listing.
          </p>
        )}
        <Price product={selected} />
        <div className="detail-description">
          <h3>Description</h3>
          <p>{selected.description || 'No description is available for this product yet.'}</p>
        </div>
        {link && !selected.removed_at ? (
          <a
            className="primary-button visit-retailer"
            href={link}
            target="_blank"
            rel="noopener noreferrer"
          >
            Visit {selected.site} <ArrowUpRight size={17} />
          </a>
        ) : (
          <p className="muted">
            {selected.removed_at
              ? 'This listing is no longer tracked. Historical changes remain available below.'
              : 'Retailer link unavailable.'}
          </p>
        )}
        <div className="detail-actions">
          {!selected.removed_at && (
            <ProductAlertButton key={`alert-${selected.id}`} product={selected} />
          )}
          <ShareButton
            title={selected.title}
            path={detailsEndpoint
              .replace('/variants/', '/variant/')
              .replace('/products/', '/product/')}
          />
        </div>
      </section>

      <section className="detail-information" aria-label="Product specifications and information">
        {componentFields.length > 0 && (
          <div className="detail-panel">
            <div className="detail-panel-heading">
              <Package size={17} />
              <div>
                <h2>Components</h2>
                <p>Configuration reported by the retailer</p>
              </div>
            </div>
            <Specifications product={selected} fields={componentFields} />
          </div>
        )}

        <div className="detail-panel product-record">
          <div className="detail-panel-heading">
            <Clock3 size={17} />
            <div>
              <h2>Product Information</h2>
              <p>Catalog record and update activity</p>
            </div>
          </div>
          <dl className="product-facts">
            <div>
              <dt>
                <Store size={13} /> Retailer
              </dt>
              <dd>{selected.site}</dd>
            </div>
            <div>
              <dt>
                <Tags size={13} /> Category
              </dt>
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
                <Clock3 size={13} /> Last Updated
              </dt>
              <dd>
                {selected.updated_at ? formatDate(selected.updated_at) : 'No updates since added'}
              </dd>
            </div>
            <div>
              <dt>
                <History size={13} /> History
              </dt>
              <dd>
                {selected.history.entries.length
                  ? `${selected.history.entries.length} recorded updates · ${selected.history.entries.reduce((total, entry) => total + entry.changes.length, 0)} changes`
                  : 'No recorded changes'}
              </dd>
            </div>
          </dl>
        </div>
        {computerSections.includes(selected.section) && (
          <section
            className="detail-panel detail-preview performance-preview"
            aria-label="Performance"
          >
            <div className="detail-panel-heading">
              <Gauge size={18} />
              <div>
                <h2>Performance</h2>
                <p>A closer look at what’s inside</p>
              </div>
              <span className="detail-coming-soon">Coming soon</span>
            </div>
            <div className="performance-preview-bars" aria-hidden="true">
              <span />
              <span />
              <span />
              <span />
              <span />
              <span />
              <span />
              <span />
            </div>
            <p className="detail-preview-copy">
              Performance insights are on the way. For now, explore the components above to compare
              configurations.
            </p>
          </section>
        )}
        <section className="detail-panel detail-preview rating-preview" aria-label="Rating">
          <div className="detail-panel-heading">
            <Star size={18} />
            <div>
              <h2>Rating</h2>
              <p>Another perspective on your next purchase</p>
            </div>
            <span className="detail-coming-soon">Coming soon</span>
          </div>
          <div className="rating-preview-stars" aria-hidden="true">
            {[0, 1, 2, 3, 4].map((value) => (
              <Star key={value} size={24} />
            ))}
          </div>
          <p className="detail-preview-copy">
            No rating yet. This space will help you look beyond the price when choosing a product.
          </p>
        </section>
      </section>
      <ProductChangeHistory key={selected.id} product={selected} />
      {sortedOffers.length > 1 && detailsEndpoint.startsWith('/variants/') && (
        <section className="offer-list detail-panel detail-listings">
          <h2>
            {sortedOffers.length} retailer {sortedOffers.length === 1 ? 'listing' : 'listings'}
          </h2>
          <p>Check each retailer’s configuration before choosing.</p>
          {sortedOffers.map((offer) => (
            <button
              className={selected.id === offer.id ? 'active' : ''}
              key={offer.id}
              onClick={(event) => {
                const dialog = event.currentTarget.closest('dialog');
                setSelected(offer);
                setZoom(false);
                requestAnimationFrame(() => {
                  dialog?.scrollTo({ top: 0, behavior: 'instant' });
                  dialog
                    ?.querySelector<HTMLElement>('#product-overview-title')
                    ?.focus({ preventScroll: true });
                });
              }}
              aria-pressed={selected.id === offer.id}
            >
              <span>
                <span className="offer-title" title={`${offer.site} · ${offer.title}`}>
                  <b>{offer.site}</b> · {offer.title}
                </span>
                <small>{stockLabels[offer.status] || 'Availability unknown'}</small>
              </span>
              <strong>{money(offer.price)} DT</strong>
              <ArrowUpRight size={16} />
            </button>
          ))}
        </section>
      )}
      {chart && <ProductAnalytics initialChart={chart} endpoint={detailsEndpoint} />}
    </div>
  );
  return embedded ? (
    content
  ) : (
    <Dialog title={selected.title} onClose={onClose} className="product-dialog">
      {content}
    </Dialog>
  );
}

export function ProductChangeHistory({ product }: { product: Product }) {
  const entries = [...product.history.entries]
    .filter((entry) => entry.changes.length > 0)
    .sort((a, b) => Date.parse(b.timestamp) - Date.parse(a.timestamp));
  const value = (field: string, raw: unknown): string => {
    if (raw === null || raw === undefined || raw === '') return 'Not recorded';
    if ((field === 'price' || field === 'original_price') && typeof raw === 'number')
      return `${money(raw)} DT`;
    if (field === 'status' && typeof raw === 'string') return stockLabels[raw] || raw;
    if (Array.isArray(raw))
      return raw.length ? raw.map((item) => value(field, item)).join(', ') : 'None';
    if (typeof raw === 'object') {
      const values = Object.entries(raw as Record<string, unknown>);
      const label = values.find(([key, item]) => key === 'label' && typeof item === 'string');
      if (label) return label[1] as string;
      return values.length
        ? values.map(([key, item]) => `${fieldLabel(key)}: ${value(key, item)}`).join(' · ')
        : 'None';
    }
    if (typeof raw === 'boolean') return raw ? 'Yes' : 'No';
    return String(raw);
  };
  return (
    <section className="detail-panel product-change-history" aria-label="Product history">
      <div className="detail-panel-heading">
        <History size={17} />
        <div>
          <h2>Product History</h2>
          <p>Recorded changes for the selected {product.site} listing · newest first</p>
        </div>
      </div>
      {entries.length ? (
        <ol className="product-timeline">
          {entries.map((entry, index) => (
            <li key={`${entry.timestamp}-${index}`}>
              <header>
                <time dateTime={entry.timestamp}>{formatDate(entry.timestamp)}</time>
                <span>
                  {entry.changes.length} {entry.changes.length === 1 ? 'field' : 'fields'} changed
                </span>
              </header>
              <ul className="product-change-list">
                {entry.changes.map((change, changeIndex) => (
                  <li key={`${change.field}-${changeIndex}`}>
                    <strong>{fieldLabel(change.field)}</strong>
                    <p>
                      <span>{value(change.field, change.old_value)}</span>
                      <span aria-label="changed to"> → </span>
                      <b>{value(change.field, change.new_value)}</b>
                    </p>
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ol>
      ) : (
        <p className="muted">
          No changes recorded yet. Price, availability, and other product updates will appear here.
        </p>
      )}
    </section>
  );
}
