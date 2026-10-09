import type { HomeProduct } from '../types';

export function retailerDiscount(product: HomeProduct) {
  if (
    product.price <= 0 ||
    product.original_price == null ||
    product.original_price <= product.price
  )
    return null;
  return {
    original: product.original_price,
    saving: product.original_price - product.price,
    percent: Math.round(((product.original_price - product.price) / product.original_price) * 100),
  };
}

export function addedAgo(timestamp: string, now: number) {
  const elapsed = Math.max(0, now - Date.parse(timestamp));
  if (!Number.isFinite(elapsed)) return '';
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return 'Added just now';
  if (minutes < 60) return `Added ${minutes} ${minutes === 1 ? 'minute' : 'minutes'} ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `Added ${hours} ${hours === 1 ? 'hour' : 'hours'} ago`;
  const days = Math.floor(hours / 24);
  return `Added ${days} ${days === 1 ? 'day' : 'days'} ago`;
}
