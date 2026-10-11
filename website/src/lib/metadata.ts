import type { Product } from '../types';
import { money, sectionLabel } from './catalog';

function setMeta(attribute: 'name' | 'property', key: string, content: string | null) {
  const existing = document.head.querySelector<HTMLMetaElement>(`meta[${attribute}="${key}"]`);
  if (content === null) {
    existing?.remove();
    return;
  }
  const element = existing || document.createElement('meta');
  element.setAttribute(attribute, key);
  element.content = content;
  if (!existing) document.head.append(element);
}

export function pageMetadata({
  title,
  description,
  path,
  image = null,
  schema = null,
  noindex = false,
}: {
  title: string;
  description: string;
  path: string;
  image?: string | null;
  schema?: object | null;
  noindex?: boolean;
}) {
  const canonical = document.head.querySelector<HTMLLinkElement>('link[rel="canonical"]');
  const base = canonical ? new URL(canonical.href).origin : window.location.origin;
  const url = new URL(path.split('?')[0], base).href;
  const previewImage = image || new URL('/icon.png', base).href;
  document.title = title;
  setMeta('name', 'description', description);
  setMeta('name', 'robots', noindex ? 'noindex, follow' : null);
  for (const [name, value] of Object.entries({
    title,
    description,
    url,
    type: 'website',
    site_name: 'Flagship',
    image: previewImage,
    'image:alt': image ? title : 'Flagship logo',
  }))
    setMeta('property', `og:${name}`, value);
  for (const [name, value] of Object.entries({
    title,
    description,
    image: previewImage,
    card: image ? 'summary_large_image' : 'summary',
  }))
    setMeta('name', `twitter:${name}`, value);
  const link = canonical || document.createElement('link');
  link.rel = 'canonical';
  link.href = url;
  if (!canonical) document.head.append(link);
  document.getElementById('flagship-structured-data')?.remove();
  if (schema) {
    const script = document.createElement('script');
    script.id = 'flagship-structured-data';
    script.type = 'application/ld+json';
    script.textContent = JSON.stringify(schema);
    document.head.append(script);
  }
}

export function productMetadata(product: Product, path: string) {
  const removed = !!product.removed_at;
  const availability =
    {
      InStock: 'In stock',
      OutOfStock: 'Out of stock',
      Arriving: 'Arriving',
      OnDemand: 'On demand',
    }[product.status] || product.status;
  const description = removed
    ? `${product.title} is no longer listed at ${product.site}. Explore its recorded history on Flagship.`
    : `${product.title} — ${product.price > 0 ? `${money(product.price)} TND` : 'Price unavailable'} at ${product.site}. ${availability}. Check recorded price history and specifications on Flagship.`;
  const image = /^https?:\/\//i.test(product.image) ? product.image : null;
  const canonical = document.head.querySelector<HTMLLinkElement>('link[rel="canonical"]');
  const url = new URL(path, canonical ? new URL(canonical.href).origin : window.location.origin)
    .href;
  const schema = {
    '@context': 'https://schema.org',
    '@type': 'Product',
    name: product.title,
    description,
    url,
    ...(image ? { image: [image] } : {}),
    ...(product.price > 0 && !removed
      ? {
          offers: {
            '@type': 'Offer',
            url,
            price: product.price,
            priceCurrency: 'TND',
            seller: { '@type': 'Organization', name: product.site },
            availability: `https://schema.org/${({ InStock: 'InStock', OutOfStock: 'OutOfStock', Arriving: 'PreOrder', OnDemand: 'BackOrder' } as Record<string, string>)[product.status] || 'OutOfStock'}`,
          },
        }
      : {}),
  };
  pageMetadata({
    title: `${product.title} — Flagship`,
    description,
    path,
    image,
    schema: removed ? null : schema,
    noindex: removed,
  });
}

export function routeMetadata(path: string) {
  const routes: Record<string, [string, string]> = {
    '/': [
      'Flagship — Tech prices in perspective',
      'Compare tech prices across Tunisian retailers. Find recorded deals, explore price history, and create free price alerts.',
    ],
    '/statistics': [
      'Market statistics — Flagship',
      'Explore comparable tech price trends, retailer comparisons, and configuration history in Tunisia.',
    ],
    '/support': [
      'Support — Flagship',
      'Meet the creator and support Flagship, a free platform for comparing tech prices in Tunisia.',
    ],
    '/future-plans': [
      'Future plans — Flagship',
      'Explore and vote for upcoming Flagship features.',
    ],
    '/legal': [
      'Terms & privacy — Flagship',
      'Simple terms of service, retailer data disclaimer, and privacy policy for Flagship.',
    ],
  };
  const [title, description] = routes[path] || [
    `${sectionLabel(path.slice(1))} — Flagship`,
    'Compare current offers, specifications, and recorded prices across Tunisian retailers on Flagship.',
  ];
  const privatePage =
    /^\/(login|register|forgot-password|reset-password|verify-email|wishlist|alerts|settings|activity)$/.test(
      path,
    );
  pageMetadata({ title, description, path, noindex: privatePage || path === '/legal' });
}
