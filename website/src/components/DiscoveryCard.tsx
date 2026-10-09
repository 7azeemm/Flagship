import { useState } from 'react';
import { ArrowRight, PackageSearch } from 'lucide-react';
import type { HomeProduct } from '../types';
import { money, sectionLabel } from '../lib/catalog';

export function ProductImage({ product }: { product: HomeProduct }) {
  const [failed, setFailed] = useState('');
  return (
    <span
      className={`home-product-image ${!product.image || failed === product.image ? 'image-unavailable' : ''}`}
    >
      {product.image && failed !== product.image ? (
        <img
          src={product.image}
          alt={product.title}
          loading="lazy"
          onError={() => setFailed(product.image)}
        />
      ) : (
        <PackageSearch aria-hidden="true" />
      )}
    </span>
  );
}

export function dealReason(product: HomeProduct) {
  const evidence = product.evidence;
  if (evidence?.new_low) return 'New 30-day low';
  if (evidence?.gap_percent != null)
    return `${evidence.gap_percent.toFixed(0)}% below next-best offer`;
  if (evidence?.recent_drop) return 'Recent price drop';
  return 'Explore this offer';
}

export function DiscoveryCard({
  product,
  onNavigate,
}: {
  product: HomeProduct;
  onNavigate: (path: string) => void;
}) {
  return (
    <a
      className="deal-card"
      href={`/product/${encodeURIComponent(product.id)}`}
      onClick={(event) => {
        if (event.button || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        onNavigate(event.currentTarget.pathname);
      }}
    >
      <ProductImage product={product} />
      <span className="discovery-evidence">{dealReason(product)}</span>
      <span className="home-product-section">{sectionLabel(product.section)}</span>
      <h3>{product.title}</h3>
      <div className="deal-price">
        <strong>
          {money(product.price)} <small>DT</small>
        </strong>
      </div>
      {product.evidence?.gap_percent != null && (
        <p className="discovery-context">
          Compared across {product.evidence.retailer_count} retailers
        </p>
      )}
      {product.evidence?.gap_percent == null && product.evidence?.typical_price != null && (
        <p className="discovery-context">
          Typical recorded price: {money(product.evidence.typical_price)} DT
        </p>
      )}
      <span className="deal-retailer">
        {product.site}
        <span>
          View offer <ArrowRight size={13} />
        </span>
      </span>
    </a>
  );
}
