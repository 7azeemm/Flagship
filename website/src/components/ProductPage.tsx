import { useEffect, useMemo, useState } from 'react';
import {
  ArrowLeft,
  ArrowUpRight,
  CalendarDays,
  Clock3,
  Maximize2,
  Package,
  RefreshCw,
} from 'lucide-react';
import type { Product, VariantChart } from '../types';
import { parseChart, parseProduct, request } from '../lib/api';
import { componentLabel, safeURL, sectionLabel, stockLabels } from '../lib/catalog';
import { Price, ProductGallery, Specifications } from './ProductCard';
import { ProductAnalytics } from './ProductAnalytics';
import { ProductChangeHistory } from './ProductDialog';
import '../product-page.css';

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

export function ProductPage({
  id,
  fields,
  onNavigate,
}: {
  id: string;
  fields: string[];
  onNavigate: (path: string) => void;
}) {
  const [product, setProduct] = useState<Product | null>(null);
  const [chart, setChart] = useState<VariantChart | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const [zoom, setZoom] = useState(false);
  const [showLoading, setShowLoading] = useState(false);
  const endpoint = `/products/${encodeURIComponent(id)}`;

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setShowLoading(false);
    setError('');
    setProduct(null);
    request(endpoint, controller.signal)
      .then((raw) => {
        if (!raw || typeof raw !== 'object') throw new Error('Product details are unavailable.');
        const record = raw as Record<string, unknown>;
        if (!record.product) throw new Error('Product details are unavailable.');
        const next = parseProduct(record.product);
        setProduct(next);
        setChart(parseChart(record.chart));
        document.title = `${next.title} — Mythos`;
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'The product could not be loaded.');
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [endpoint, revision]);

  useEffect(() => {
    if (!loading) return;
    const timer = window.setTimeout(() => setShowLoading(true), 500);
    return () => window.clearTimeout(timer);
  }, [loading]);

  const componentFields = useMemo(() => {
    if (!product) return fields;
    const available = fields.filter((field) => componentLabel(product.components[field]).trim());
    const configured = new Set(fields);
    const extras = Object.keys(product.components).filter(
      (field) =>
        !configured.has(field) && componentLabel(product.components[field]).trim().length > 0,
    );
    return [...available, ...extras];
  }, [fields, product]);

  if (loading)
    return (
      <main className="product-page product-page-state" id="product-content" tabIndex={-1}>
        {showLoading && (
          <div className="product-page-skeleton" aria-label="Loading product details" />
        )}
      </main>
    );

  if (!product || error)
    return (
      <main className="product-page product-page-state" id="product-content" tabIndex={-1}>
        <div className="product-page-error" role="alert">
          <RefreshCw size={28} />
          <h1>We couldn’t open this product.</h1>
          <p>{error || 'The product may no longer be available.'}</p>
          <button className="primary-button" onClick={() => setRevision((value) => value + 1)}>
            Try again
          </button>
        </div>
      </main>
    );

  const retailerLink = safeURL(product.url);
  const categoryPath = `/${product.section}`;

  return (
    <main className="product-page" id="product-content" tabIndex={-1}>
      <nav className="product-breadcrumb" aria-label="Breadcrumb">
        <a
          href={categoryPath}
          onClick={(event) => {
            if (!event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey) {
              event.preventDefault();
              onNavigate(categoryPath);
            }
          }}
        >
          <ArrowLeft size={15} /> {sectionLabel(product.section)}
        </a>
        <span>/</span>
        <span>{product.title}</span>
      </nav>

      <article className="product-page-card">
        <div className="product-detail-grid product-page-grid">
          <section className="detail-overview" aria-labelledby="product-page-title">
            <ProductGallery
              product={product}
              className={`detail-image ${zoom ? 'zoomed' : ''}`}
              onOpen={() => setZoom(!zoom)}
              label={zoom ? 'Reduce product image' : 'Enlarge product image'}
              pressed={zoom}
              eager
            >
              <Maximize2 size={18} />
            </ProductGallery>
            <div className="detail-meta">
              <span className="retailer-badge">{product.site}</span>
              <span className={`stock stock-${product.status}`}>
                {stockLabels[product.status] || 'Availability unknown'}
              </span>
            </div>
            <h1 id="product-page-title">{product.title}</h1>
            <Price product={product} />
            <div className="detail-description">
              <h2>Description</h2>
              <p>{product.description || 'No description is available for this product yet.'}</p>
            </div>
            {retailerLink ? (
              <a
                className="primary-button visit-retailer"
                href={retailerLink}
                target="_blank"
                rel="noopener noreferrer"
              >
                Visit {product.site} <ArrowUpRight size={17} />
              </a>
            ) : (
              <p className="muted">Retailer link unavailable.</p>
            )}
          </section>

          <section
            className="detail-information"
            aria-label="Product specifications and information"
          >
            {componentFields.length > 0 && (
              <div className="detail-panel">
                <div className="detail-panel-heading">
                  <Package size={17} />
                  <div>
                    <h2>Components</h2>
                    <p>Configuration reported by the retailer</p>
                  </div>
                </div>
                <Specifications product={product} fields={componentFields} />
              </div>
            )}

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
                  <dd>{product.site}</dd>
                </div>
                <div>
                  <dt>Category</dt>
                  <dd>{sectionLabel(product.section)}</dd>
                </div>
                <div>
                  <dt>
                    <CalendarDays size={13} /> Added
                  </dt>
                  <dd>{formatDate(product.added_at)}</dd>
                </div>
                <div>
                  <dt>
                    <Clock3 size={13} /> Last updated
                  </dt>
                  <dd>{formatDate(product.updated_at)}</dd>
                </div>
              </dl>
            </div>
          </section>

          <ProductChangeHistory product={product} />
          <ProductAnalytics initialChart={chart} endpoint={endpoint} product={product} />
        </div>
      </article>
    </main>
  );
}
