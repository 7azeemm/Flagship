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
  ComponentDetails,
  GlobalSearchData,
} from '../types';
import { defaultStockStatuses } from './catalog';

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
export function parseComponentDetails(value: unknown): ComponentDetails {
  if (
    !record(value) ||
    (value.kind !== 'CPU' && value.kind !== 'GPU') ||
    typeof value.id !== 'string' ||
    typeof value.label !== 'string' ||
    !record(value.specifications) ||
    !Object.values(value.specifications).every(
      (item) => typeof item === 'string' || typeof item === 'boolean' || number(item),
    )
  )
    throw invalid();
  return value as unknown as ComponentDetails;
}
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
    removed_at: typeof value.removed_at === 'string' ? value.removed_at : null,
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
      typeof value.representative_product_id === 'string' ? value.representative_product_id : '',
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
      retailer_prices: record(point.retailer_prices)
        ? (Object.fromEntries(
            Object.entries(point.retailer_prices)
              .filter(([, prices]) => record(prices))
              .map(([retailer, prices]) => [
                retailer,
                Object.fromEntries(
                  Object.entries(prices as Record<string, unknown>).filter(
                    ([, price]) => number(price) && price > 0,
                  ),
                ),
              ]),
          ) as Record<string, Record<string, number>>)
        : null,
      status_counts: record(point.status_counts)
        ? (Object.fromEntries(
            Object.entries(point.status_counts).filter(([, count]) => number(count) && count >= 0),
          ) as Record<string, number>)
        : null,
      timestamp: point.timestamp,
      lowest_price: point.lowest_price as number | null,
      lowest_price_retailer: point.lowest_price_retailer as string | null,
      offer_count: point.offer_count,
      in_stock_count: point.in_stock_count,
      retailer_count: point.retailer_count,
    };
  });
  return {
    combination_fields: record(value.combination_fields)
      ? Object.fromEntries(
          Object.entries(value.combination_fields).filter(
            (entry): entry is [string, string] => typeof entry[1] === 'string',
          ),
        )
      : {},
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
          const url = new URL(`/${path.replace(/^\//, '')}`, 'https://flagship.local');
          const href = url.pathname + url.search;
          if (urls.has(href)) throw invalid();
          urls.add(href);
          return {
            id: url.pathname.slice(1),
            name: child.name,
            url: href,
            ...(child.featured === true ? { featured: true } : {}),
          };
        }),
      ];
    }),
  );
  const defaults = record(value.defaults) ? value.defaults : {};
  return {
    categories,
    defaults: {
      stock: Array.isArray(defaults.stock)
        ? defaults.stock.filter((v): v is string => typeof v === 'string')
        : [...defaultStockStatuses],
      section:
        typeof defaults.section === 'string'
          ? defaults.section
          : Object.values(categories).flat()[0]?.id || '',
      sort:
        defaults.sort === 'newest'
          ? 'newest'
          : defaults.sort === 'best_deals'
            ? 'best_deals'
            : defaults.sort === 'discount_desc'
              ? 'discount_desc'
              : defaults.sort === 'price_desc'
                ? 'price_desc'
                : 'price_asc',
    },
  };
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
  return { ...value, evidence: parseDealEvidence(value.evidence) } as unknown as HomeProduct;
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
    featured_sections: Array.isArray(value.featured_sections)
      ? value.featured_sections.filter((v): v is string => typeof v === 'string')
      : [],
    market_today: parseMarketToday(value.market_today),
    market_views: marketViews,
    deals: value.deals.map(parseHomeProduct),
    new_products: value.new_products.map(parseHomeProduct),
    retailers,
  };
}

function parseMarketToday(value: unknown): HomepageData['market_today'] {
  if (!record(value) || typeof value.date !== 'string' || !number(value.price_drops))
    throw invalid();
  return {
    date: value.date,
    price_drops: value.price_drops,
    biggest_drop: value.biggest_drop === null ? null : parseHomeProduct(value.biggest_drop),
  };
}

export function parseGlobalSearch(value: unknown): GlobalSearchData {
  if (
    !record(value) ||
    typeof value.query !== 'string' ||
    !Array.isArray(value.items) ||
    !number(value.total_items) ||
    !number(value.total_pages) ||
    !number(value.page)
  )
    throw invalid();
  return {
    query: value.query,
    items: value.items.map(parseProduct),
    total_items: value.total_items,
    total_pages: value.total_pages,
    page: value.page,
  };
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
    !number(value.configuration_page) ||
    !number(value.configuration_pages) ||
    !Array.isArray(value.category_trends) ||
    !Array.isArray(value.highlights) ||
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
        const inStock = optionalCount(item.in_stock_offers);
        if (inStock !== undefined && inStock > item.offers) throw invalid();
        return {
          name: item.name,
          offers: item.offers,
          in_stock_offers: inStock,
          lowest_price_wins: item.lowest_price_wins,
          price_comparisons: item.price_comparisons,
          within_two_percent: optionalCount(item.within_two_percent),
          two_to_five_percent: optionalCount(item.two_to_five_percent),
          above_five_percent: optionalCount(item.above_five_percent),
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
      return {
        timestamp: point.timestamp,
        value: point.value,
        sample_size: point.sample_size,
        break_before: point.break_before === true,
      };
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
      !nullableNumber(item.current_price) ||
      !number(item.in_stock) ||
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
        return {
          timestamp: point.timestamp,
          value: point.value,
          sample_size: point.sample_size,
          break_before: point.break_before === true,
        };
      });
    return {
      id: item.id,
      title: item.title,
      product_id: item.product_id,
      section: typeof item.section === 'string' ? item.section : undefined,
      offers: item.offers,
      retailer_count: item.retailer_count,
      current_price: item.current_price,
      in_stock: item.in_stock,
      price_points: points(item.price_points),
      availability_points: points(item.availability_points),
    };
  });
  if (value.matching_products != null && !Array.isArray(value.matching_products)) throw invalid();
  if (
    value.catalog_filters != null &&
    (!record(value.catalog_filters) || !Object.values(value.catalog_filters).every(strings))
  )
    throw invalid();
  return {
    catalog_filters:
      value.catalog_filters == null ? null : (value.catalog_filters as Record<string, string[]>),
    matching_products:
      value.matching_products == null ? null : value.matching_products.map(parseHomeProduct),
    supported_sections: Array.isArray(value.supported_sections)
      ? value.supported_sections.filter((section): section is string => typeof section === 'string')
      : [],
    generated_at: value.generated_at,
    last_scan_at: value.last_scan_at,
    price_changes: parsePriceChanges(value.price_changes),
    configuration_summary: parseConfigurationSummary(value.configuration_summary),
    offer_summary: parseOfferSummary(value.offer_summary),
    retailer_overviews: retailerOverviews,
    explorer_charts: value.explorer_charts.map(chart),
    facets,
    retailers: value.retailers,
    configuration_count: value.configuration_count,
    configuration_page: value.configuration_page,
    configuration_pages: value.configuration_pages,
    highlights: value.highlights.map(parseHomeProduct),
    category_trends: value.category_trends.map((item: unknown) => {
      if (
        !record(item) ||
        typeof item.section !== 'string' ||
        !nullableNumber(item.change_percent) ||
        !number(item.comparable_offers) ||
        !nullableString(item.since)
      )
        throw invalid();
      return {
        section: item.section,
        change_percent: item.change_percent,
        comparable_offers: item.comparable_offers,
        since: item.since,
      };
    }),
    configurations,
  };
}

function optionalCount(value: unknown): number | undefined {
  if (value === undefined) return undefined;
  if (!number(value) || !Number.isInteger(value) || value < 0) throw invalid();
  return value;
}

export function parsePriceChanges(raw: unknown): StatisticsData['price_changes'] {
  if (raw === undefined || raw === null) return null;
  if (
    !record(raw) ||
    !nullableString(raw.since) ||
    typeof raw.until !== 'string' ||
    !['fell', 'rose', 'unchanged'].includes(String(raw.selected)) ||
    !Array.isArray(raw.offers)
  )
    throw invalid();
  for (const key of [
    'compared_offers',
    'excluded_offers',
    'fell',
    'rose',
    'unchanged',
    'page',
    'pages',
  ]) {
    if (optionalCount(raw[key]) === undefined) throw invalid();
  }
  if (
    Number(raw.fell) + Number(raw.rose) + Number(raw.unchanged) !== raw.compared_offers ||
    Number(raw.page) < 1 ||
    Number(raw.pages) < Number(raw.page) ||
    !Number.isFinite(Date.parse(raw.until)) ||
    (raw.since !== null && !Number.isFinite(Date.parse(raw.since)))
  )
    throw invalid();
  const offers = raw.offers.map((offer: unknown) => {
    if (
      !record(offer) ||
      typeof offer.id !== 'string' ||
      typeof offer.title !== 'string' ||
      typeof offer.retailer !== 'string' ||
      !number(offer.before) ||
      offer.before <= 0 ||
      !number(offer.after) ||
      offer.after <= 0 ||
      !number(offer.change_percent) ||
      typeof offer.in_stock !== 'boolean'
    )
      throw invalid();
    return {
      id: offer.id,
      title: offer.title,
      retailer: offer.retailer,
      before: offer.before,
      after: offer.after,
      change_percent: offer.change_percent,
      in_stock: offer.in_stock,
    };
  });
  const summary = parsePriceChangeSummary(raw.summary);
  if (summary && (summary.fell.count !== raw.fell || summary.rose.count !== raw.rose))
    throw invalid();
  let magnitude_bands: NonNullable<StatisticsData['price_changes']>['magnitude_bands'] = null;
  if (raw.magnitude_bands !== undefined && raw.magnitude_bands !== null) {
    const boundaries = [0, 2, 5, 10, 20];
    if (!Array.isArray(raw.magnitude_bands) || raw.magnitude_bands.length !== boundaries.length)
      throw invalid();
    magnitude_bands = raw.magnitude_bands.map((band: unknown, index: number) => {
      if (
        !record(band) ||
        band.from !== boundaries[index] ||
        band.to !== (boundaries[index + 1] ?? null) ||
        optionalCount(band.fell) === undefined ||
        optionalCount(band.rose) === undefined
      )
        throw invalid();
      return {
        from: Number(band.from),
        to: band.to as number | null,
        fell: Number(band.fell),
        rose: Number(band.rose),
      };
    });
    if (
      magnitude_bands.reduce((sum, band) => sum + band.fell, 0) !== raw.fell ||
      magnitude_bands.reduce((sum, band) => sum + band.rose, 0) !== raw.rose
    )
      throw invalid();
  }
  return { ...raw, offers, summary, magnitude_bands } as NonNullable<
    StatisticsData['price_changes']
  >;
}

function parsePriceChangeSummary(
  raw: unknown,
): NonNullable<StatisticsData['price_changes']>['summary'] {
  if (raw === undefined || raw === null) return null;
  if (!record(raw)) throw invalid();
  const direction = (
    value: unknown,
    sign: number,
  ): import('../types').PriceChangeDirectionSummary => {
    if (!record(value) || optionalCount(value.count) === undefined) throw invalid();
    const fields = [
      'average_percent',
      'average_amount_dt',
      'largest_percent',
      'largest_amount_dt',
    ] as const;
    if (value.count === 0) {
      if (fields.some((field) => value[field] !== null)) throw invalid();
    } else {
      if (fields.some((field) => !number(value[field]) || Number(value[field]) * sign <= 0))
        throw invalid();
      if (
        Math.abs(Number(value.largest_percent)) + 1e-9 < Math.abs(Number(value.average_percent)) ||
        Math.abs(Number(value.largest_amount_dt)) + 1e-9 <
          Math.abs(Number(value.average_amount_dt)) ||
        !Number.isInteger(value.largest_amount_dt) ||
        (sign < 0 && Number(value.largest_percent) <= -100)
      )
        throw invalid();
    }
    return {
      count: Number(value.count),
      average_percent: value.average_percent as number | null,
      average_amount_dt: value.average_amount_dt as number | null,
      largest_percent: value.largest_percent as number | null,
      largest_amount_dt: value.largest_amount_dt as number | null,
    };
  };
  return { fell: direction(raw.fell, -1), rose: direction(raw.rose, 1) };
}

export function parseOfferSummary(raw: unknown): StatisticsData['offer_summary'] {
  if (raw == null) return null;
  if (
    !record(raw) ||
    !Array.isArray(raw.price_bands) ||
    [
      'offer_count',
      'available_offers',
      'priced_offers',
      'available_priced_offers',
      'retailer_count',
    ].some((key) => optionalCount(raw[key]) === undefined)
  )
    throw invalid();
  const total = Number(raw.offer_count);
  const available = Number(raw.available_offers);
  const priced = Number(raw.priced_offers);
  const availablePriced = Number(raw.available_priced_offers);
  const retailers = Number(raw.retailer_count);
  if (
    available > total ||
    priced > total ||
    availablePriced > available ||
    availablePriced > priced ||
    retailers > total ||
    (total > 0 && retailers === 0) ||
    (priced === 0
      ? raw.average_price !== null
      : !number(raw.average_price) || raw.average_price <= 0)
  )
    throw invalid();
  const bands = raw.price_bands.map((band: unknown) => {
    if (
      !record(band) ||
      optionalCount(band.from) === undefined ||
      optionalCount(band.count) === undefined ||
      (band.to !== null &&
        (optionalCount(band.to) === undefined || Number(band.to) <= Number(band.from)))
    )
      throw invalid();
    return { from: Number(band.from), to: band.to as number | null, count: Number(band.count) };
  });
  if (
    bands.reduce((sum, band) => sum + band.count, 0) !== availablePriced ||
    (availablePriced > 0 && !bands.length) ||
    bands.some((band, index) =>
      index === 0 ? band.from !== 0 : bands[index - 1].to !== band.from,
    ) ||
    (bands.length > 0 && bands.at(-1)!.to !== null)
  )
    throw invalid();
  return {
    offer_count: total,
    available_offers: available,
    priced_offers: priced,
    available_priced_offers: availablePriced,
    retailer_count: retailers,
    average_price: raw.average_price as number | null,
    price_bands: bands,
  };
}

export function parseConfigurationSummary(raw: unknown): StatisticsData['configuration_summary'] {
  if (raw === undefined || raw === null) return null;
  if (
    !record(raw) ||
    ['configuration_count', 'available_configurations', 'shared_configurations'].some(
      (key) => optionalCount(raw[key]) === undefined,
    ) ||
    !Array.isArray(raw.price_bands) ||
    !Array.isArray(raw.spread_bands)
  )
    throw invalid();
  const configurationCount = Number(raw.configuration_count);
  const available = Number(raw.available_configurations);
  const shared = Number(raw.shared_configurations);
  if (shared > available || available > configurationCount) throw invalid();
  for (const field of ['average_spread_percent', 'average_spread_dt']) {
    if (shared === 0 ? raw[field] !== null : !number(raw[field]) || Number(raw[field]) < 0)
      throw invalid();
  }
  const priceBands = raw.price_bands.map((value: unknown) => {
    if (
      !record(value) ||
      optionalCount(value.from) === undefined ||
      optionalCount(value.count) === undefined ||
      (value.to !== null &&
        (optionalCount(value.to) === undefined || Number(value.to) <= Number(value.from)))
    )
      throw invalid();
    return { from: Number(value.from), to: value.to as number | null, count: Number(value.count) };
  });
  if (
    priceBands.reduce((total, band) => total + band.count, 0) !== available ||
    (available > 0 && priceBands.length === 0) ||
    priceBands.some((band, index) =>
      index === 0 ? band.from !== 0 : priceBands[index - 1].to !== band.from,
    ) ||
    (priceBands.length > 0 && priceBands[priceBands.length - 1].to !== null)
  )
    throw invalid();
  const spreadBands = raw.spread_bands.map((value: unknown) => {
    if (
      !record(value) ||
      typeof value.label !== 'string' ||
      optionalCount(value.count) === undefined
    )
      throw invalid();
    return { label: value.label, count: Number(value.count) };
  });
  if (
    spreadBands.reduce((total, band) => total + band.count, 0) !== shared ||
    new Set(spreadBands.map((band) => band.label)).size !== spreadBands.length
  )
    throw invalid();
  return {
    configuration_count: configurationCount,
    available_configurations: available,
    shared_configurations: shared,
    average_spread_percent: raw.average_spread_percent as number | null,
    average_spread_dt: raw.average_spread_dt as number | null,
    price_bands: priceBands,
    spread_bands: spreadBands,
  };
}

export function parseDealEvidence(value: unknown): import('../types').DealEvidence | null {
  if (
    !record(value) ||
    !['new_low', 'recent_drop', 'enough_history'].every((key) => typeof value[key] === 'boolean') ||
    !['typical_price', 'history_saving_percent', 'next_best_price', 'gap_percent'].every((key) =>
      nullableNumber(value[key]),
    ) ||
    !nullableString(value.next_best_retailer) ||
    !number(value.retailer_count) ||
    !record(value.comparison_fields) ||
    !Object.values(value.comparison_fields).every((item) => typeof item === 'string')
  )
    return null;
  return value as unknown as import('../types').DealEvidence;
}
export function parseDeals(value: unknown): Record<string, import('../types').DealEvidence> {
  if (!record(value)) return {};
  return Object.fromEntries(
    Object.entries(value).flatMap(([id, raw]) => {
      const deal = parseDealEvidence(raw);
      return deal ? [[id, deal]] : [];
    }),
  );
}
