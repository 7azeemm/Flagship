import type { Product, VariantChart, VariantChartPoint } from '../types';

// Older APIs return no chart when a product cannot join a variant. Its own record
// still contains the initial observation and change log, so it remains chartable.
export function buildProductHistoryChart(
  product: Product,
  range: string,
  interval: string,
  now = Date.now(),
): VariantChart {
  let price = product.price;
  let stock = product.status === 'InStock' ? 1 : 0;
  const snapshot = (timestamp: number): VariantChartPoint => ({
    timestamp: new Date(timestamp).toISOString(),
    lowest_price: price,
    lowest_price_retailer: product.site,
    offer_count: 1,
    in_stock_count: stock,
    retailer_count: 1,
  });
  const events = product.history.entries
    .filter(
      (entry) =>
        Number.isFinite(Date.parse(entry.timestamp)) &&
        entry.changes.some((change) => ['price', 'status'].includes(change.field)),
    )
    .sort((a, b) => Date.parse(b.timestamp) - Date.parse(a.timestamp));
  const history: VariantChartPoint[] = [];
  for (const event of events) {
    history.push(snapshot(Date.parse(event.timestamp)));
    for (const change of event.changes) {
      if (
        change.field === 'price' &&
        typeof change.old_value === 'number' &&
        Number.isFinite(change.old_value)
      )
        price = change.old_value;
      if (change.field === 'status' && typeof change.old_value === 'string')
        stock = change.old_value === 'InStock' ? 1 : 0;
    }
  }
  const added = Date.parse(product.added_at || '');
  history.push(snapshot(Number.isFinite(added) ? added : now));
  history.reverse();
  history.push({
    timestamp: new Date(now).toISOString(),
    lowest_price: product.price,
    lowest_price_retailer: product.site,
    offer_count: 1,
    in_stock_count: product.status === 'InStock' ? 1 : 0,
    retailer_count: 1,
  });
  const days = (
    { '7d': 7, '30d': 30, '90d': 90, '180d': 180, '1y': 365 } as Record<string, number>
  )[range];
  const since = days ? now - days * 86400000 : -Infinity;
  const points = history.filter((p) => Date.parse(p.timestamp) >= since);
  const baseline = history.filter((p) => Date.parse(p.timestamp) < since).at(-1);
  if (baseline) points.unshift({ ...baseline, timestamp: new Date(since).toISOString() });
  const resolved =
    interval === 'auto'
      ? range === 'all'
        ? 'week'
        : ['180d', '1y'].includes(range)
          ? 'day'
          : 'scan'
      : interval;
  const buckets = new Map<string, VariantChartPoint>();
  for (const point of points) {
    const date = new Date(point.timestamp);
    if (resolved === 'week') date.setUTCDate(date.getUTCDate() - ((date.getUTCDay() + 6) % 7));
    buckets.set(date.toISOString().slice(0, 10), point);
  }
  const prices = points.map((p) => p.lowest_price!);
  const ninetySince = now - 90 * 86400000;
  const ninetyPoints = history.filter((point) => Date.parse(point.timestamp) >= ninetySince);
  const ninetyBaseline = history
    .filter((point) => Date.parse(point.timestamp) < ninetySince)
    .at(-1);
  if (ninetyBaseline) ninetyPoints.unshift(ninetyBaseline);
  const ninetyDayPrices = ninetyPoints.map((point) => point.lowest_price!);
  return {
    scope: 'product',
    range,
    interval: resolved,
    currency: 'TND',
    points: resolved === 'scan' ? points : [...buckets.values()],
    summary: {
      current_price: product.price,
      period_low: Math.min(...prices),
      period_high: Math.max(...prices),
      all_time_low: Math.min(...history.map((p) => p.lowest_price!)),
      average_price: Math.trunc(prices.reduce((a, b) => a + b, 0) / prices.length),
      ninety_day_average: Math.trunc(
        ninetyDayPrices.reduce((a, b) => a + b, 0) / ninetyDayPrices.length,
      ),
    },
  };
}
