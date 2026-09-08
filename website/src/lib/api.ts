import type {
  CatalogItem,
  FilterValue,
  PageResponse,
  Product,
  SectionInfo,
  ServerInfo,
  VariantChart,
  VariantSummary,
} from '../types';

const origin = '/api';
const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);
const strings = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');
const stringMap = (value: unknown): Record<string, string> =>
  record(value)
    ? Object.fromEntries(
        Object.entries(value).filter(
          (entry): entry is [string, string] => typeof entry[1] === 'string',
        ),
      )
    : {};
const number = (value: unknown): value is number =>
  typeof value === 'number' && Number.isFinite(value);
const invalid = () => new Error('The catalog returned an unexpected response. Please try again.');

export function parseProduct(value: unknown): Product {
  if (
    !record(value) ||
    typeof value.id !== 'string' ||
    typeof value.title !== 'string' ||
    !number(value.price)
  )
    throw invalid();
  return {
    id: value.id,
    title: value.title,
    price: value.price,
    url: typeof value.url === 'string' ? value.url : '',
    image: typeof value.image === 'string' ? value.image : '',
    site: typeof value.site === 'string' ? value.site : 'Retailer',
    section: typeof value.section === 'string' ? value.section : '',
    status: typeof value.status === 'string' ? value.status : 'Unknown',
    original_price: number(value.original_price) ? value.original_price : null,
    description: typeof value.description === 'string' ? value.description : null,
    components: stringMap(value.components),
    filter_ids: stringMap(value.filter_ids),
    history: Array.isArray(value.history) ? value.history : [],
    updated_at: typeof value.updated_at === 'string' ? value.updated_at : null,
    added_at: typeof value.added_at === 'string' ? value.added_at : null,
    views: number(value.views) ? value.views : 0,
    missing_group_fields: strings(value.missing_group_fields) ? value.missing_group_fields : [],
  };
}
function parseVariant(value: unknown): VariantSummary {
  if (
    !record(value) ||
    typeof value.id !== 'string' ||
    typeof value.section !== 'string' ||
    !number(value.price) ||
    !number(value.offer_count) ||
    !number(value.retailer_count) ||
    !number(value.in_stock_count)
  )
    throw invalid();
  return {
    id: value.id,
    name: value.id,
    section: value.section,
    price: value.price,
    image: typeof value.image === 'string' ? value.image : '',
    original_price: number(value.original_price) ? value.original_price : null,
    components: stringMap(value.components),
    offer_count: value.offer_count,
    retailer_count: value.retailer_count,
    in_stock_count: value.in_stock_count,
    views: number(value.views) ? value.views : 0,
  };
}
export function parseChart(value: unknown): VariantChart | null {
  if (
    !record(value) ||
    typeof value.range !== 'string' ||
    typeof value.interval !== 'string' ||
    typeof value.currency !== 'string' ||
    !Array.isArray(value.points) ||
    !record(value.summary)
  )
    return null;
  const nullableNumber = (item: unknown) => item === null || number(item);
  if (
    !nullableNumber(value.summary.current_price) ||
    !nullableNumber(value.summary.period_low) ||
    !nullableNumber(value.summary.period_high) ||
    !nullableNumber(value.summary.all_time_low) ||
    !nullableNumber(value.summary.average_price) ||
    !nullableNumber(value.summary.ninety_day_average)
  )
    return null;
  const points = value.points.map((point: unknown) => {
    if (
      !record(point) ||
      typeof point.timestamp !== 'string' ||
      !Number.isFinite(Date.parse(point.timestamp)) ||
      !nullableNumber(point.lowest_price) ||
      !number(point.offer_count) ||
      !number(point.in_stock_count) ||
      !number(point.retailer_count) ||
      (point.lowest_price_retailer !== null && typeof point.lowest_price_retailer !== 'string')
    )
      throw invalid();
    return {
      timestamp: point.timestamp,
      lowest_price: point.lowest_price as number | null,
      lowest_price_retailer: point.lowest_price_retailer as string | null,
      offer_count: point.offer_count,
      in_stock_count: point.in_stock_count,
      retailer_count: point.retailer_count,
    };
  });
  return {
    scope: value.scope === 'product' ? 'product' : 'variant',
    range: value.range,
    interval: value.interval,
    currency: value.currency,
    points,
    summary: value.summary as VariantChart['summary'],
  };
}
function parseNode(value: unknown, depth = 0): FilterValue {
  if (
    depth > 20 ||
    !record(value) ||
    typeof value.id !== 'string' ||
    typeof value.label !== 'string' ||
    !number(value.count)
  )
    throw invalid();
  return {
    id: value.id,
    label: value.label,
    count: value.count,
    children: Array.isArray(value.children)
      ? value.children.map((child) => parseNode(child, depth + 1))
      : [],
  };
}
export function parsePage(value: unknown): PageResponse {
  if (
    !record(value) ||
    !Array.isArray(value.items) ||
    !number(value.total_items) ||
    !number(value.total_products) ||
    !number(value.total_pages) ||
    !number(value.page)
  )
    throw invalid();
  let info: SectionInfo | null = null;
  if (value.section_info != null) {
    const raw = value.section_info;
    if (
      !record(raw) ||
      !Array.isArray(raw.filters) ||
      !strings(raw.sites) ||
      !strings(raw.components) ||
      !Array.isArray(raw.group_by) ||
      !number(raw.min_price) ||
      !number(raw.max_price)
    )
      throw invalid();
    info = {
      sites: raw.sites,
      components: raw.components,
      components_by_section: record(raw.components_by_section)
        ? Object.fromEntries(
            Object.entries(raw.components_by_section).filter((entry): entry is [string, string[]] =>
              strings(entry[1]),
            ),
          )
        : {},
      group_by: raw.group_by.map((field: unknown) => {
        if (!record(field) || typeof field.key !== 'string' || typeof field.required !== 'boolean')
          throw invalid();
        return { key: field.key, required: field.required };
      }),
      min_price: raw.min_price,
      max_price: raw.max_price,
      filters: raw.filters.map((group: unknown) => {
        if (
          !record(group) ||
          typeof group.key !== 'string' ||
          typeof group.label !== 'string' ||
          !Array.isArray(group.options)
        )
          throw invalid();
        return {
          key: group.key,
          label: group.label,
          options: group.options.map((node) => parseNode(node)),
        };
      }),
    };
  }
  return {
    section_info: info,
    items: value.items.map((item: unknown): CatalogItem => {
      if (!record(item) || (item.type !== 'product' && item.type !== 'variant')) throw invalid();
      return item.type === 'product'
        ? { type: 'product', data: parseProduct(item.data) }
        : { type: 'variant', data: parseVariant(item.data) };
    }),
    total_items: value.total_items,
    total_products: value.total_products,
    total_pages: value.total_pages,
    page: value.page,
  };
}
export async function request(path: string, signal: AbortSignal): Promise<unknown> {
  try {
    const response = await fetch(`${origin}${path}`, {
      signal: AbortSignal.any([signal, AbortSignal.timeout(15000)]),
      headers: { Accept: 'application/json' },
    });
    if (!response.ok) throw new Error('The catalog is temporarily unavailable. Please try again.');
    return await response.json();
  } catch (error) {
    if (signal.aborted) throw error;
    if (error instanceof SyntaxError) throw invalid();
    if (error instanceof Error && error.name === 'TimeoutError')
      throw new Error('The catalog is taking too long to respond. Please try again.');
    if (error instanceof TypeError)
      throw new Error('We couldn’t reach the catalog. Check your connection and try again.');
    throw error;
  }
}
export function parseServerInfo(value: unknown): ServerInfo {
  if (!record(value)) throw invalid();
  const source = record(value.categories) ? value.categories : value;
  const urls = new Set<string>();
  const categories = Object.fromEntries(
    Object.entries(source).map(([parent, children]) => {
      if (!Array.isArray(children)) throw invalid();
      return [
        parent,
        children.map((child: unknown) => {
          if (!record(child) || typeof child.name !== 'string') throw invalid();
          const path = typeof child.url === 'string' ? child.url : child.id;
          if (typeof path !== 'string' || !/^\/?[A-Za-z]+(?:\?[^#]*)?$/.test(path)) throw invalid();
          const url = new URL(`/${path.replace(/^\//, '')}`, 'https://mythos.local');
          const href = url.pathname + url.search;
          if (urls.has(href)) throw invalid();
          urls.add(href);
          return { id: url.pathname.slice(1), name: child.name, url: href };
        }),
      ];
    }),
  );
  return { categories };
}
