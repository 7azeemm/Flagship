export interface Category {
  id: string;
  name: string;
  url: string;
}
export interface ServerInfo {
  categories: Record<string, Category[]>;
}
export interface StructuredComponent {
  id: string;
  label: string;
}
export type ComponentValue = string | StructuredComponent;
export interface Product {
  id: string;
  url: string;
  image: string;
  images: string[];
  status: string;
  site: string;
  section: string;
  title: string;
  price: number;
  original_price: number | null;
  description: string | null;
  components: Record<string, ComponentValue>;
  history: ProductHistory;
  updated_at: string | null;
  added_at: string | null;
  views: number;
  missing_group_fields: string[];
}
export interface ProductHistory {
  entries: ProductHistoryEntry[];
}
export interface ProductHistoryEntry {
  timestamp: string;
  changes: ProductHistoryChange[];
}
export interface ProductHistoryChange {
  field: string;
  old_value: unknown;
  new_value: unknown;
}
export interface VariantSummary {
  id: string;
  section: string;
  image: string;
  images: string[];
  price: number;
  original_price: number | null;
  components: Record<string, ComponentValue>;
  offer_count: number;
  retailer_count: number;
  in_stock_count: number;
  views: number;
}
export type CatalogItem =
  { type: 'product'; data: Product } | { type: 'variant'; data: VariantSummary };
export interface VariantChartPoint {
  timestamp: string;
  lowest_price: number | null;
  lowest_price_retailer: string | null;
  offer_count: number;
  in_stock_count: number;
  retailer_count: number;
}
export interface VariantChart {
  scope?: 'product' | 'variant';
  range: string;
  interval: string;
  currency: string;
  points: VariantChartPoint[];
  summary: {
    current_price: number | null;
    period_low: number | null;
    period_high: number | null;
    all_time_low: number | null;
    average_price: number | null;
    ninety_day_average: number | null;
  };
}
export interface FilterValue {
  id?: string;
  label: string;
  count: number;
  children?: FilterValue[];
}
export interface FilterGroup {
  key: string;
  label: string;
  options: FilterValue[];
}
export interface FacetCount {
  value: string;
  count: number;
}
export interface SectionInfo {
  filters: FilterGroup[];
  sites: FacetCount[];
  statuses: FacetCount[];
  components: string[];
  group_by: { key: string; required: boolean }[];
  min_price: number;
  max_price: number;
}
export interface PageResponse {
  section_info: SectionInfo | null;
  items: CatalogItem[];
  total_items: number;
  total_products: number;
  total_pages: number;
  page: number;
}
export interface CatalogQuery {
  section: string;
  search: string;
  min_price: string;
  max_price: string;
  site: string[];
  stock: string[];
  filters: Record<string, string[]>;
  sort: 'price_asc' | 'price_desc';
  grouped: boolean;
  groupBy: string[] | null;
  page: number;
}

export interface HomeProduct {
  id: string;
  title: string;
  section: string;
  image: string;
  price: number;
  original_price: number | null;
  previous_price: number | null;
  saving: number | null;
  change_percent: number | null;
  site: string;
  status: string;
  views: number;
  added_at: string;
  updated_at: string | null;
  badge: string | null;
}

export interface HomepageData {
  generated_at: string;
  coverage: {
    products: number;
    retailers: number;
    in_stock_offers: number;
    updated_at: string | null;
    last_scan_at: string | null;
    next_scan_at: string | null;
  };
  hero_deal: HomeProduct | null;
  market_views: {
    range: string;
    section: string | null;
    change_percent: number | null;
    tracked_offers: number;
    comparable_products: number;
    price_decreases: number;
    price_increases: number;
    in_stock_percent: number;
    points: { timestamp: string; index: number; comparable_products: number }[];
  }[];
  deals: HomeProduct[];
  new_products: HomeProduct[];
  retailers: { name: string; products: number; in_stock: number }[];
}

export interface StatisticPoint {
  timestamp: string;
  value: number;
  sample_size: number;
}

export interface StatisticChart {
  id: string;
  title: string;
  description: string;
  section: string;
  filters: Record<string, string>;
  metric: 'index' | 'median_price' | 'lowest_price' | 'availability';
  range: string;
  matching_offers: number;
  retailer_count: number;
  comparable_offers: number;
  points: StatisticPoint[];
  note: string;
}

export interface StatisticsData {
  generated_at: string;
  last_scan_at: string | null;
  retailer_overviews: Record<string, {
    comparable_configurations: number;
    retailers: {
      name: string;
      offers: number;
      lowest_price_wins: number;
      price_comparisons: number;
    }[];
  }>;
  explorer_charts: StatisticChart[];
  facets: { key: string; label: string; options: { value: string; count: number }[] }[];
  retailers: string[];
  configuration_count: number;
  configurations: {
    id: string;
    title: string;
    product_id: string | null;
    offers: number;
    retailer_count: number;
    price_points: StatisticPoint[];
    availability_points: StatisticPoint[];
  }[];
}
