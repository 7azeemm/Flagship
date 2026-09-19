import type { CatalogQuery, ComponentValue, FilterValue, Product } from '../types';

const labels: Record<string, string> = {
  PC: 'Desktop PCs',
  GamingPC: 'Gaming PCs',
  AllInOnePC: 'All-in-one PCs',
  MiniPC: 'Mini PCs',
  Laptop: 'Laptops',
  GamingLaptop: 'Gaming laptops',
  MacBook: 'MacBooks',
  Monitor: 'Monitors',
  CPU: 'Processors',
  GPU: 'Graphics cards',
  Memory: 'Memory',
  Storage: 'Storage',
  Motherboard: 'Motherboards',
  PowerSupply: 'Power supplies',
  Mouse: 'Mice',
  Keyboard: 'Keyboards',
  MousePad: 'Mouse pads',
  GamingChair: 'Gaming chairs',
  AccessoriesCombo: 'Accessory bundles',
  UpgradeKit: 'Upgrade kits',
  Console: 'Consoles',
  Controller: 'Controllers',
  ConsoleGame: 'Console games',
  ConsoleAccessories: 'Console accessories',
  Smartphone: 'Smartphones',
  Tablet: 'Tablets',
  Smartwatch: 'Smartwatches',
  Television: 'Televisions',
  Cooler: 'Cooling',
  Case: 'PC cases',
  Others: 'Other products',
};
export const sectionLabel = (key: string) => labels[key] || key.replace(/([a-z])([A-Z])/g, '$1 $2');
export const fieldLabel = (key: string) =>
  (
    ({
      cpu: 'CPU',
      gpu: 'GPU',
      memory: 'RAM',
      ram: 'RAM',
      model: 'Model',
      display: 'Display',
      storage: 'Storage',
    }) as Record<string, string>
  )[key] || key.replaceAll('_', ' ').replace(/^./, (c) => c.toUpperCase());
export const stockLabels: Record<string, string> = {
  InStock: 'In stock',
  OutOfStock: 'Out of stock',
  OnArrive: 'Arriving soon',
  Arriving: 'Arriving soon',
  OnRequest: 'On demand',
  OnDemand: 'On demand',
};
export const money = (value: number) =>
  new Intl.NumberFormat('en-TN', { maximumFractionDigits: 0 }).format(value);
export const defaultQuery = (section = 'GamingLaptop'): CatalogQuery => ({
  section,
  search: '',
  min_price: '',
  max_price: '',
  site: [],
  stock: ['InStock'],
  filters: {},
  sort: 'price_asc',
  grouped: null,
  groupBy: null,
  page: 1,
});
export function readQuery(location: Pick<Location, 'pathname' | 'search'>): CatalogQuery {
  const params = new URLSearchParams(location.search);
  const path = location.pathname.replace(/^\/+|\/+$/g, '');
  const section =
    Object.keys(labels).find((key) => key.toLowerCase() === path.toLowerCase()) ||
    (/^[A-Za-z]+$/.test(path) ? path : 'GamingLaptop');
  const price = (key: string) => {
    const value = params.get(key);
    return value && /^\d+$/.test(value) && Number(value) <= 2147483647 ? value : '';
  };
  let filters: Record<string, string[]> = {};
  try {
    const raw: unknown = JSON.parse(params.get('filters') || '{}');
    if (raw && typeof raw === 'object' && !Array.isArray(raw))
      filters = Object.fromEntries(
        Object.entries(raw).filter(
          (entry): entry is [string, string[]] =>
            Array.isArray(entry[1]) && entry[1].every((value) => typeof value === 'string'),
        ),
      );
  } catch {
    /* Invalid shared parameters are ignored. */
  }
  return {
    ...defaultQuery(section),
    search: params.get('search') || '',
    min_price: price('min_price'),
    max_price: price('max_price'),
    site: params.getAll('site'),
    stock: params.has('stock')
      ? params.getAll('stock')
      : params.get('availability') === 'all'
        ? []
        : ['InStock'],
    filters,
    groupBy: (() => {
      try {
        const value: unknown = JSON.parse(params.get('group_by') || 'null');
        return Array.isArray(value) && value.every((key) => typeof key === 'string') ? value : null;
      } catch {
        return null;
      }
    })(),
    grouped: params.has('grouping_mode') ? params.get('grouping_mode') !== 'false' : null,
    sort: params.get('sort') === 'price_desc' ? 'price_desc' : 'price_asc',
    page: Math.min(1000000, Math.max(1, parseInt(params.get('page') || '1', 10) || 1)),
  };
}
export function queryParams(query: CatalogQuery, metadata = false) {
  const params = new URLSearchParams();
  for (const key of ['search', 'min_price', 'max_price'] as const)
    if (query[key]) params.set(key, query[key]);
  for (const key of ['site', 'stock'] as const)
    query[key].forEach((value) => params.append(key, value));
  if (query.sort !== 'price_asc') params.set('sort', query.sort);
  if (query.page > 1) params.set('page', String(query.page));
  if (!query.stock.length) params.set('availability', 'all');
  if (query.groupBy !== null) params.set('group_by', JSON.stringify(query.groupBy));
  if (query.grouped !== null) params.set('grouping_mode', String(query.grouped));
  const filters = Object.fromEntries(Object.entries(query.filters).filter(([, values]) => values.length));
  if (Object.keys(filters).length) params.set('filters', JSON.stringify(filters));
  if (metadata) params.set('send_section_info', 'true');
  return params;
}
export function queryURL(query: CatalogQuery) {
  const params = queryParams(query).toString();
  return `/${query.section}${params ? `?${params}` : ''}`;
}
export const toggle = (values: string[], value: string) =>
  values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
const rulePath = (rule: string) => rule.replace(/^!/, '');
export const filterValue = (node: FilterValue) => node.id || node.label;
export const isWithin = (value: string, parent: string) =>
  value === parent || value.startsWith(`${parent}/`);
function selectedAt(value: string, selected: string[]) {
  const rule = selected
    .filter((candidate) => isWithin(value, rulePath(candidate)))
    .sort((a, b) => rulePath(b).length - rulePath(a).length)[0];
  return !!rule && !rule.startsWith('!');
}
export function nodeStatus(node: FilterValue, selected: string[]): 'checked' | 'partial' | 'unchecked' {
  const value = filterValue(node);
  if (!node.id) return selected.includes(value) ? 'checked' : 'unchecked';
  const checked = selectedAt(value, selected);
  const changedBelow = selected.some(
    (rule) =>
      rulePath(rule) !== value &&
      isWithin(rulePath(rule), value) &&
      !rule.startsWith('!') !== checked,
  );
  return changedBelow ? 'partial' : checked ? 'checked' : 'unchecked';
}
export function toggleNode(selected: string[], node: FilterValue): string[] {
  const value = filterValue(node);
  if (!node.id)
    return selected.includes(value)
      ? selected.filter((selectedValue) => selectedValue !== value)
      : [...selected, value];
  const checked = selectedAt(value, selected);
  const rest = selected.filter((rule) => !isWithin(rulePath(rule), value));
  const inherited = selectedAt(value, rest);
  return checked ? (inherited ? [...rest, `!${value}`] : rest) : inherited ? rest : [...rest, value];
}
export const hasFilterChanges = (query: CatalogQuery) =>
  !!(
    query.search ||
    query.min_price ||
    query.max_price ||
    query.site.length ||
    Object.values(query.filters).some((values) => values.length) ||
    query.stock.length !== 1 ||
    query.stock[0] !== 'InStock'
  );
export function safeURL(value: string): string | undefined {
  try {
    const url = new URL(value);
    return ['https:', 'http:'].includes(url.protocol) ? url.href : undefined;
  } catch {
    return undefined;
  }
}
export function productSpecs(product: Product, fields: string[]) {
  return fields.map((key) => {
    const value = componentLabel(product.components[key]).trim();
    return { key, label: fieldLabel(key), value: value || 'Unknown', unknown: !value };
  });
}
export function componentLabel(value: ComponentValue | undefined): string {
  return typeof value === 'string' ? value : value?.label || '';
}
export function pageNumbers(page: number, total: number) {
  const start = Math.max(1, page - 3);
  const end = Math.min(total, page + 3);
  return [
    ...new Set([
      1,
      ...Array.from({ length: Math.max(0, end - start + 1) }, (_, index) => start + index),
      total,
    ]),
  ].filter((value) => value >= 1 && value <= total);
}
export function clampRange(low: number, high: number, min: number, max: number): [number, number] {
  const floor = Math.max(0, Math.min(min, max));
  const ceiling = Math.max(floor, max);
  const left = Math.max(floor, Math.min(Number.isFinite(low) ? low : floor, ceiling));
  return [left, Math.max(left, Math.min(Number.isFinite(high) ? high : ceiling, ceiling))];
}

// Metadata depends on selected filters, never on result presentation.
export function sectionInfoKey(query: CatalogQuery) {
  return JSON.stringify([
    query.section,
    query.search,
    query.min_price,
    query.max_price,
    [...query.site].sort(),
    [...query.stock].sort(),
    Object.entries(query.filters)
      .filter(([, values]) => values.length)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, values]) => [key, [...values].sort()]),
  ]);
}
export function apiQueryParams(query: CatalogQuery, metadata: boolean) {
  const params = queryParams(query, metadata);
  params.delete('group_by');
  if (query.grouped !== false) (query.groupBy ?? []).forEach((key) => params.append('group_by', key));
  return params;
}

export const groupingEnabled = (query: CatalogQuery, info: { group_by_default: boolean } | null) =>
  query.grouped ?? info?.group_by_default ?? false;
