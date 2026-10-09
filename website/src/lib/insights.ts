import type { StatisticsData } from '../types';

export const minimumRetailerComparisons = 5;

export function retailerLandscape(
  retailers: StatisticsData['retailer_overviews'][string]['retailers'],
) {
  return rankRetailers(retailers, 'price')
    .filter((item) => item.in_stock_offers !== undefined && item.within_two_percent !== undefined)
    .map((item) => ({ ...item, inStock: item.in_stock_offers! }));
}

export function rankRetailers(
  retailers: StatisticsData['retailer_overviews'][string]['retailers'],
  measure: InsightQuery['retailerMeasure'],
) {
  return retailers
    .filter((item) =>
      measure === 'offers'
        ? (item.in_stock_offers ?? 0) > 0
        : item.price_comparisons >= minimumRetailerComparisons,
    )
    .map((item) => ({
      ...item,
      competitiveShare:
        item.price_comparisons > 0
          ? (100 * (item.lowest_price_wins + (item.within_two_percent || 0))) /
            item.price_comparisons
          : 0,
      value:
        measure === 'offers'
          ? item.in_stock_offers!
          : retailerRankingScore(
              item.lowest_price_wins + (item.within_two_percent || 0),
              item.price_comparisons,
            ),
    }))
    .sort(
      (a, b) =>
        b.value - a.value ||
        b.price_comparisons - a.price_comparisons ||
        a.name.localeCompare(b.name),
    );
}

export const insightRanges = [
  ['7d', '7 days'],
  ['30d', '30 days'],
  ['90d', '90 days'],
  ['180d', '6 months'],
  ['1y', '1 year'],
  ['all', 'All time'],
] as const;
export type InsightView = 'overview' | 'retailers' | 'explorer';
export interface InsightQuery {
  view: InsightView;
  section: string;
  range: string;
  metric: 'index' | 'price_changes' | 'availability' | 'lowest_price';
  retailerMeasure: 'price' | 'offers';
  retailer: string;
  filters: Record<string, string>;
}
export function readInsightQuery(search: string): InsightQuery {
  const p = new URLSearchParams(search);
  let filters: Record<string, string> = {};
  try {
    const raw: unknown = JSON.parse(p.get('filters') || '{}');
    if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
      filters = Object.fromEntries(
        Object.entries(raw).filter(
          ([key, value]) => key.length < 80 && typeof value === 'string' && value.length <= 100,
        ),
      );
    }
  } catch {
    /* Malformed shared filters fall back to an unfiltered view. */
  }
  const view = p.get('view');
  const metric = p.get('metric');
  const retailerMeasure = p.get('retailer_measure');
  const section = p.get('section') || 'All';
  return {
    view: view === 'retailers' || view === 'explorer' ? view : 'overview',
    section,
    range: insightRanges.some(([value]) => value === p.get('range')) ? p.get('range')! : '30d',
    metric:
      metric === 'price_changes' || metric === 'median_price'
        ? 'price_changes'
        : metric === 'availability' || metric === 'lowest_price'
          ? metric
          : 'index',
    // Legacy lowest/competitiveness links now open the unified price ranking.
    retailerMeasure: retailerMeasure === 'offers' ? 'offers' : 'price',
    retailer: p.get('retailer') || '',
    filters: section === 'All' ? {} : filters,
  };
}
export function insightParams(query: InsightQuery) {
  const p = new URLSearchParams({ view: query.view, section: query.section, range: query.range });
  if (query.metric !== 'index') p.set('metric', query.metric);
  if (query.retailerMeasure !== 'price') p.set('retailer_measure', query.retailerMeasure);
  if (query.retailer) p.set('retailer', query.retailer);
  if (query.section !== 'All' && Object.keys(query.filters).length)
    p.set('filters', JSON.stringify(query.filters));
  return p;
}
export function statisticsAPIParams(query: InsightQuery) {
  const params = new URLSearchParams({
    section: query.section,
    range: query.range,
    individual_offers: 'true',
  });
  if (query.view === 'explorer') {
    if (query.section !== 'All') params.set('filters', JSON.stringify(query.filters));
    if (query.retailer) params.set('retailer', query.retailer);
  }
  return params;
}
export function signedPercent(value: number) {
  return `${value > 0.05 ? '+' : ''}${Math.abs(value) < 0.05 ? '0.0' : value.toFixed(1)}%`;
}
// Lower Wilson bound: small perfect samples should not outrank stronger evidence.
// This is a ranking score, not the retailer's observed win percentage.
export function retailerRankingScore(wins: number, comparisons: number) {
  if (comparisons <= 0) return 0;
  const rate = Math.max(0, Math.min(wins, comparisons)) / comparisons;
  const z = 1.96;
  const z2 = z * z;
  return (
    (100 *
      (rate +
        z2 / (2 * comparisons) -
        z * Math.sqrt((rate * (1 - rate) + z2 / (4 * comparisons)) / comparisons))) /
    (1 + z2 / comparisons)
  );
}
export function shortDate(value: string) {
  return new Date(value).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
}
export function historyLabel(points: { timestamp: string }[]) {
  if (!points.length) return 'No supported history yet';
  if (points.length === 1) return `One observation · ${shortDate(points[0].timestamp)}`;
  return `Available history · ${shortDate(points[0].timestamp)} – ${shortDate(points.at(-1)!.timestamp)}`;
}
export function priceTick(value: number) {
  return `${new Intl.NumberFormat('en', { maximumFractionDigits: 1 }).format(value)} DT`;
}
