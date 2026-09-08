import type { Product, VariantChartPoint } from '../types';
import { productHistoryChart } from './productHistory';

export interface RetailerSeries {
  site: string;
  points: VariantChartPoint[];
  current: number | null;
  low: number | null;
  high: number | null;
}

// Compare the cheapest in-stock listing per retailer, without carrying prices
// across periods when that retailer had no available offer.
export function retailerHistory(
  products: Product[],
  range: string,
  interval: string,
  now = Date.now(),
): RetailerSeries[] {
  const sites = [...new Set(products.map((product) => product.site))].sort();
  return sites.map((site) => {
    const offers = products.filter((product) => product.site === site);
    const histories = offers.map(
      (product) => productHistoryChart(product, range, 'scan', now).points,
    );
    const times = [
      ...new Set(histories.flatMap((points) => points.map((point) => point.timestamp))),
    ].sort();
    const cursors = histories.map(() => -1);
    const points = times.map((timestamp) => {
      const snapshots = histories
        .map((history, index) => {
          while (
            cursors[index] + 1 < history.length &&
            history[cursors[index] + 1].timestamp <= timestamp
          )
            cursors[index]++;
          return history[cursors[index]];
        })
        .filter((point) => point !== undefined);
      const prices = snapshots
        .filter((point) => point.in_stock_count > 0 && point.lowest_price !== null)
        .map((point) => point.lowest_price!);
      return {
        timestamp,
        lowest_price: prices.length ? Math.min(...prices) : null,
        lowest_price_retailer: site,
        offer_count: snapshots.length,
        in_stock_count: prices.length,
        retailer_count: prices.length ? 1 : 0,
      };
    });
    const prices = points.flatMap((point) =>
      point.lowest_price === null ? [] : [point.lowest_price],
    );
    const current = offers
      .filter((offer) => offer.status === 'InStock')
      .map((offer) => offer.price);
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
    return {
      site,
      points: resolved === 'scan' ? points : [...buckets.values()],
      current: current.length ? Math.min(...current) : null,
      low: prices.length ? Math.min(...prices) : null,
      high: prices.length ? Math.max(...prices) : null,
    };
  });
}
