import { useEffect, useState } from 'react';
import type { Product, VariantChart, DealEvidence } from '../types';
import { parseChart, parseProduct, parseDeals, request } from '../lib/api';
import { Dialog } from './Dialog';
import { ProductDialog } from './ProductDialog';
import '../product-page.css';
import { pageMetadata, productMetadata } from '../lib/metadata';

// Both navigation inside the app and a fresh shared URL use this loader.
export function ProductPage({
  id,
  kind,
  initialProductId,
  computerSections,
  onClose,
}: {
  id: string;
  kind: 'product' | 'variant';
  initialProductId?: string;
  computerSections: string[];
  onClose: () => void;
}) {
  const [details, setDetails] = useState<{
    deals: Record<string, DealEvidence>;
    offers: Product[];
    chart: VariantChart | null;
    preferred?: string;
  } | null>(null);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const endpoint =
    '/' + (kind === 'variant' ? 'variants' : 'products') + '/' + encodeURIComponent(id);

  useEffect(() => {
    if (!details) return;
    const path = `/${kind}/${encodeURIComponent(id)}`;
    if (kind === 'product') productMetadata(details.offers[0], path);
    else {
      const name = Object.values(details.chart?.combination_fields || {}).join(' · ') || 'Product configuration';
      pageMetadata({ title: `${name} — Flagship`, description: `Compare ${details.offers.length} offers for ${name} across Tunisian retailers. Explore prices, availability, and configuration history on Flagship.`, path, image: details.offers.find(product => /^https?:\/\//i.test(product.image))?.image || null });
    }
  }, [details, id, kind]);

  useEffect(() => {
    const controller = new AbortController();
    setError('');
    setDetails(null);
    void request(endpoint, controller.signal)
      .then((raw) => {
        if (controller.signal.aborted) return;
        const record = raw as {
          deals?: unknown;
          product?: unknown;
          products?: unknown[];
          chart?: unknown;
          variant?: { representative_product_id?: string };
        };
        const offers =
          kind === 'variant'
            ? Array.isArray(record?.products)
              ? record.products.map(parseProduct)
              : []
            : record?.product
              ? [parseProduct(record.product)]
              : [];
        if (!offers.length) throw new Error('No retailer listings are available.');
        setDetails({
          deals: parseDeals(record.deals),
          offers,
          chart: parseChart(record.chart),
          preferred: record.variant?.representative_product_id,
        });
      })
      .catch((cause) => {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'Product details could not be loaded.');
      });
    return () => controller.abort();
  }, [endpoint, kind, revision]);

  return (
    <Dialog title="Product details" onClose={onClose} className="product-dialog">
      {!details ? (
        <div
          className="product-page-error product-detail-loading"
          role={error ? 'alert' : 'status'}
        >
          <p>{error || 'Loading product details…'}</p>
          {error && (
            <button className="primary-button" onClick={() => setRevision((value) => value + 1)}>
              Try again
            </button>
          )}
        </div>
      ) : (
        <ProductDialog
          computerSections={computerSections}
          embedded
          offers={details.offers}
          chart={details.chart}
          deals={details.deals}
          fields={[]}
          detailsEndpoint={endpoint}
          initialProductId={initialProductId || details.preferred}
          onClose={onClose}
          onSelectProduct={(product) => {
            if (kind !== 'variant') return;
            const url = new URL(window.location.href);
            url.searchParams.set('product', product.id);
            window.history.replaceState(window.history.state, '', url);
          }}
        />
      )}
    </Dialog>
  );
}
