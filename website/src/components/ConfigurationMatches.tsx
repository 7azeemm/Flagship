import { ArrowRight, ImageOff } from 'lucide-react';
import { useState } from 'react';
import type { HomeProduct } from '../types';
import { defaultQuery, money, queryURL, safeURL, sectionLabel, stockLabels } from '../lib/catalog';

function MatchingProduct({
  product,
  onNavigate,
}: {
  product: HomeProduct;
  onNavigate: (path: string) => void;
}) {
  const [failedImage, setFailedImage] = useState('');
  const image = safeURL(product.image);
  return (
    <a
      className="configuration-match"
      href={`/product/${encodeURIComponent(product.id)}`}
      onClick={(event) => {
        if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        onNavigate(event.currentTarget.pathname);
      }}
    >
      <span className="configuration-match-image">
        {image && failedImage !== image ? (
          <img
            src={image}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            onError={() => setFailedImage(image)}
          />
        ) : (
          <ImageOff size={22} aria-hidden="true" />
        )}
      </span>
      <span className="configuration-match-info">
        <small>
          {sectionLabel(product.section)} · {product.site}
        </small>
        <strong title={product.title}>{product.title}</strong>
        <span className="configuration-match-price">
          {money(product.price)} <small>DT</small>
          <small className={product.status === 'InStock' ? 'is-available' : ''}>
            {stockLabels[product.status] || product.status}
          </small>
        </span>
      </span>
    </a>
  );
}

export function ConfigurationMatches({
  products,
  section,
  retailer,
  filters,
  catalogFilters,
  onNavigate,
}: {
  products: HomeProduct[] | null;
  section: string;
  retailer: string;
  filters: Record<string, string>;
  catalogFilters: Record<string, string[]> | null;
  onNavigate: (path: string) => void;
}) {
  if (!products?.length) return null;
  const catalogPath =
    section === 'All' ||
    (Object.keys(filters).length > 0 &&
      (!catalogFilters || Object.values(catalogFilters).some((values) => !values.length)))
      ? null
      : queryURL({
          ...defaultQuery(section),
          grouped: false,
          site: retailer ? [retailer] : [],
          filters: catalogFilters || {},
        });
  return (
    <section className="configuration-matches" aria-labelledby="configuration-matches-title">
      <div className="insight-section-heading">
        <div>
          <h2 id="configuration-matches-title">Matching products</h2>
          <p>
            In-stock offers first ·{' '}
            {section === 'All' ? 'recently added examples' : 'lowest prices shown'}
          </p>
        </div>
        {catalogPath && (
          <a
            className="insight-text-action"
            href={catalogPath}
            onClick={(event) => {
              if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
                return;
              event.preventDefault();
              onNavigate(catalogPath);
            }}
          >
            View all matches <ArrowRight size={15} />
          </a>
        )}
      </div>
      <div className="configuration-match-grid">
        {products.slice(0, 4).map((product) => (
          <MatchingProduct key={product.id} product={product} onNavigate={onNavigate} />
        ))}
      </div>
    </section>
  );
}
