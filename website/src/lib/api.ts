import type {
  CatalogItem,
  FilterValue,
  PageResponse,
  Product,
  SectionInfo,
  ServerInfo,
  VariantChart,
  VariantSummary,
  HomeProduct,
  HomepageData,
  StatisticsData,
  ComponentValue,
} from '../types';

const origin = '/api';
const record = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === 'object' && !Array.isArray(value);
const strings = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');
const componentMap = (value: unknown): Record<string, ComponentValue> => {
  if (!record(value)) return {};
  const components: [string, ComponentValue][] = [];
  for (const [key, component] of Object.entries(value)) {
    if (typeof component === 'string') components.push([key, component]);
    else if (
      record(component) &&
      typeof component.id === 'string' &&
      typeof component.label === 'string'
    )
      components.push([key, { id: component.id, label: component.label }]);
  }
  return Object.fromEntries(components);
};
const number = (value: unknown): value is number =>
  typeof value === 'number' && Number.isFinite(value);
const invalid = () => new Error('The catalog returned an unexpected response. Please try again.');
const facetCounts = (value: unknown) => {
  if (
    !Array.isArray(value) ||
    !value.every((item) => record(item) && typeof item.value === 'string' && number(item.count))
  )
    throw invalid();
  return value.map((item) => ({ value: item.value as string, count: item.count as number }));
};

const productHistory = (value: unknown): Product['history'] => {
  if (value === undefined || value === null) return { entries: [] };
  const entries = Array.isArray(value)
    ? value
    : record(value) && Array.isArray(value.entries)
      ? value.entries
      : null;
  if (!entries) throw invalid();
  return {
    entries: entries.map((entry) => {
      if (
        !record(entry) ||
        typeof entry.timestamp !== 'string' ||
        !Number.isFinite(Date.parse(entry.timestamp)) ||
        !Array.isArray(entry.changes)
      )
        throw invalid();
      return {
        timestamp: entry.timestamp,
        changes: entry.changes.map((change) => {
          if (
            !record(change) ||
            typeof change.field !== 'string' ||
            !Object.hasOwn(change, 'old_value') ||
            !Object.hasOwn(change, 'new_value')
          )
            throw invalid();
          return {
            field: change.field,
            old_value: change.old_value,
            new_value: change.new_value,
          };
        }),
      };
    }),
  };
};

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
    images: strings(value.images) ? value.images : [],
    site:
      typeof value.site === 'string'
        ? value.site
        : typeof value.retailer === 'string'
          ? value.retailer
          : 'Retailer',
    section: typeof value.section === 'string' ? value.section : '',
    status: typeof value.status === 'string' ? value.status : 'Unknown',
    original_price: number(value.original_price) ? value.original_price : null,
    description: typeof value.description === 'string' ? value.description : null,
    components: componentMap(value.components),
    history: productHistory(value.history),
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
    title: typeof value.title === 'string' ? value.title : value.id,
    representative_product_id:
      typeof value.representative_product_id === 'string'
        ? value.representative_product_id
        : '',
    section: value.section,
    price: value.price,
    image: typeof value.image === 'string' ? value.image : '',
    images: strings(value.images) ? value.images : [],
    original_price: number(value.original_price) ? value.original_price : null,
    components: componentMap(value.components),
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
    (value.id !== undefined && typeof value.id !== 'string') ||
    typeof value.label !== 'string' ||
    !number(value.count)
  )
    throw invalid();
  return {
    ...(typeof value.id === 'string' ? { id: value.id } : {}),
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
      !strings(raw.components) ||
      !Array.isArray(raw.group_by) ||
      typeof raw.group_by_default !== 'boolean' ||
      !number(raw.min_price) ||
      !number(raw.max_price)
    )
      throw invalid();
    info = {
      sites: facetCounts(raw.sites),
      statuses: facetCounts(raw.statuses),
      components: raw.components,
      group_by_default: raw.group_by_default,
      group_by: raw.group_by.map((field: unknown) => {
        if (
          !record(field) ||
          typeof field.key !== 'string' ||
          typeof field.required !== 'boolean' ||
          typeof field.default !== 'boolean'
        )
          throw invalid();
        return { key: field.key, required: field.required, default: field.default };
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

const nullableNumber = (value: unknown): value is number | null => value === null || number(value);
const nullableString = (value: unknown): value is string | null =>
  value === null || typeof value === 'string';

export function parseHomeProduct(value: unknown): HomeProduct {
  if (
    !record(value) ||
    typeof value.id !== 'string' ||
    typeof value.title !== 'string' ||
    typeof value.section !== 'string' ||
    typeof value.image !== 'string' ||
    !number(value.price) ||
    !nullableNumber(value.original_price) ||
    !nullableNumber(value.previous_price) ||
    !nullableNumber(value.saving) ||
    !nullableNumber(value.change_percent) ||
    typeof value.site !== 'string' ||
    typeof value.status !== 'string' ||
    !number(value.views) ||
    typeof value.added_at !== 'string' ||
    !nullableString(value.updated_at) ||
    !nullableString(value.badge)
  )
    throw invalid();
  return value as unknown as HomeProduct;
}

export function parseHomepage(value: unknown): HomepageData {
  if (
    !record(value) ||
    typeof value.generated_at !== 'string' ||
    !record(value.coverage) ||
    !number(value.coverage.products) ||
    !number(value.coverage.retailers) ||
    !number(value.coverage.in_stock_offers) ||
    !nullableString(value.coverage.updated_at) ||
    !nullableString(value.coverage.last_scan_at) ||
    !nullableString(value.coverage.next_scan_at) ||
    !Array.isArray(value.market_views) ||
    value.market_views.length === 0 ||
    !Array.isArray(value.deals) ||
    !Array.isArray(value.new_products) ||
    !Array.isArray(value.retailers)
  )
    throw invalid();
  const marketViews = value.market_views.map((view) => {
    if (
      !record(view) ||
      typeof view.range !== 'string' ||
      !nullableString(view.section) ||
      !nullableNumber(view.change_percent) ||
      !number(view.tracked_offers) ||
      !number(view.comparable_products) ||
      !number(view.price_decreases) ||
      !number(view.price_increases) ||
      !number(view.in_stock_percent) ||
      !Array.isArray(view.points)
    )
      throw invalid();
    const points = view.points.map((point) => {
      if (
        !record(point) ||
        typeof point.timestamp !== 'string' ||
        !Number.isFinite(Date.parse(point.timestamp)) ||
        !number(point.index) ||
        !number(point.comparable_products)
      )
        throw invalid();
      return {
        timestamp: point.timestamp,
        index: point.index,
        comparable_products: point.comparable_products,
      };
    });
    return { ...view, points } as HomepageData['market_views'][number];
  });
  const retailers = value.retailers.map((item) => {
    if (
      !record(item) ||
      typeof item.name !== 'string' ||
      !number(item.products) ||
      !number(item.in_stock)
    )
      throw invalid();
    return {
      name: item.name,
      products: item.products,
      in_stock: item.in_stock,
    };
  });
  return {
    generated_at: value.generated_at,
    coverage: value.coverage as unknown as HomepageData['coverage'],
    hero_deal: value.hero_deal === null ? null : parseHomeProduct(value.hero_deal),
    market_views: marketViews,
    deals: value.deals.map(parseHomeProduct),
    new_products: value.new_products.map(parseHomeProduct),
    retailers,
  };
}

export function parseSearchSuggestions(value: unknown): HomeProduct[] {
  if (!Array.isArray(value)) throw invalid();
  return value.map(parseHomeProduct);
}

export function parseStatistics(value: unknown): StatisticsData {
  if (
    !record(value) ||
    typeof value.generated_at !== 'string' ||
    !nullableString(value.last_scan_at) ||
    !record(value.retailer_overviews) ||
    !Array.isArray(value.explorer_charts) ||
    value.explorer_charts.length !== 4 ||
    !Array.isArray(value.facets) ||
    !strings(value.retailers) ||
    !number(value.configuration_count) ||
    !Array.isArray(value.configurations)
  )
    throw invalid();
  const retailerOverviews: StatisticsData['retailer_overviews'] = {};
  for (const [section, overview] of Object.entries(value.retailer_overviews)) {
    if (
      !record(overview) ||
      !number(overview.comparable_configurations) ||
      !Array.isArray(overview.retailers)
    )
      throw invalid();
    retailerOverviews[section] = {
      comparable_configurations: overview.comparable_configurations,
      retailers: overview.retailers.map((item: unknown) => {
        if (
          !record(item) ||
          typeof item.name !== 'string' ||
          !number(item.offers) ||
          !number(item.lowest_price_wins) ||
          !number(item.price_comparisons)
        )
          throw invalid();
        return {
          name: item.name,
          offers: item.offers,
          lowest_price_wins: item.lowest_price_wins,
          price_comparisons: item.price_comparisons,
        };
      }),
    };
  }
  const chart = (raw: unknown): StatisticsData['explorer_charts'][number] => {
    if (
      !record(raw) ||
      typeof raw.id !== 'string' ||
      typeof raw.title !== 'string' ||
      typeof raw.description !== 'string' ||
      typeof raw.section !== 'string' ||
      !record(raw.filters) ||
      typeof raw.metric !== 'string' ||
      typeof raw.range !== 'string' ||
      !number(raw.matching_offers) ||
      !number(raw.retailer_count) ||
      !number(raw.comparable_offers) ||
      !Array.isArray(raw.points) ||
      typeof raw.note !== 'string'
    )
      throw invalid();
    if (!['index', 'median_price', 'lowest_price', 'availability'].includes(raw.metric))
      throw invalid();
    const points = raw.points.map((point: unknown) => {
      if (
        !record(point) ||
        typeof point.timestamp !== 'string' ||
        !Number.isFinite(Date.parse(point.timestamp)) ||
        !number(point.value) ||
        !number(point.sample_size)
      )
        throw invalid();
      return { timestamp: point.timestamp, value: point.value, sample_size: point.sample_size };
    });
    return { ...raw, points } as StatisticsData['explorer_charts'][number];
  };
  const facets = value.facets.map((facet: unknown) => {
    if (
      !record(facet) ||
      typeof facet.key !== 'string' ||
      typeof facet.label !== 'string' ||
      !Array.isArray(facet.options)
    )
      throw invalid();
    return {
      key: facet.key,
      label: facet.label,
      options: facet.options.map((option: unknown) => {
        if (!record(option) || typeof option.value !== 'string' || !number(option.count))
          throw invalid();
        return { value: option.value, count: option.count };
      }),
    };
  });
  const configurations = value.configurations.map((item: unknown) => {
    if (
      !record(item) ||
      typeof item.id !== 'string' ||
      typeof item.title !== 'string' ||
      !nullableString(item.product_id) ||
      !number(item.offers) ||
      !number(item.retailer_count) ||
      !Array.isArray(item.price_points) ||
      !Array.isArray(item.availability_points)
    )
      throw invalid();
    const points = (raw: unknown[]) =>
      raw.map((point) => {
        if (
          !record(point) ||
          typeof point.timestamp !== 'string' ||
          !Number.isFinite(Date.parse(point.timestamp)) ||
          !number(point.value) ||
          !number(point.sample_size)
        )
          throw invalid();
        return { timestamp: point.timestamp, value: point.value, sample_size: point.sample_size };
      });
    return {
      id: item.id,
      title: item.title,
      product_id: item.product_id,
      offers: item.offers,
      retailer_count: item.retailer_count,
      price_points: points(item.price_points),
      availability_points: points(item.availability_points),
    };
  });
  return {
    generated_at: value.generated_at,
    last_scan_at: value.last_scan_at,
    retailer_overviews: retailerOverviews,
    explorer_charts: value.explorer_charts.map(chart),
    facets,
    retailers: value.retailers,
    configuration_count: value.configuration_count,
    configurations,
  };
}
