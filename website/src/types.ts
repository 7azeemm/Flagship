export interface Category {
  id: string;
  name: string;
  url: string;
  featured?: boolean;
}
export interface ServerInfo {
  categories: Record<string, Category[]>;
  defaults: {
    stock: string[];
    section: string;
    sort: 'price_asc' | 'price_desc' | 'discount_desc' | 'best_deals' | 'newest';
  };
}
export interface StructuredComponent {
  id: string;
  label: string;
}
export type ComponentValue = string | StructuredComponent;
export interface ComponentDetails {
  kind: 'CPU' | 'GPU';
  id: string;
  label: string;
  specifications: Record<string, string | number | boolean>;
}
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
  removed_at?: string | null;
  listing_history?: { timestamp: string; listed: boolean }[];
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
  title: string;
  representative_product_id: string;
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
  retailer_prices?: Record<string, Record<string, number>> | null;
  status_counts?: Record<string, number> | null;
  timestamp: string;
  lowest_price: number | null;
  lowest_price_retailer: string | null;
  offer_count: number;
  in_stock_count: number;
  retailer_count: number;
}
export interface VariantChart {
  combination_fields?: Record<string, string>;
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
  group_by: { key: string; required: boolean; default: boolean }[];
  group_by_default: boolean;
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
export interface GlobalSearchData {
  query: string;
  items: Product[];
  total_items: number;
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
  sort: 'price_asc' | 'price_desc' | 'discount_desc' | 'best_deals' | 'newest';
  grouped: boolean | null;
  groupBy: string[] | null;
  page: number;
}

export interface DealEvidence {
  new_low: boolean;
  recent_drop: boolean;
  enough_history: boolean;
  typical_price: number | null;
  history_saving_percent: number | null;
  next_best_price: number | null;
  next_best_retailer: string | null;
  retailer_count: number;
  gap_percent: number | null;
  comparison_fields: Record<string, string>;
}

export interface HomeProduct {
  evidence?: DealEvidence | null;
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
  featured_sections: string[];
  market_today: { date: string; price_drops: number; biggest_drop: HomeProduct | null };
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
  break_before?: boolean;
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
  offer_summary: {
    offer_count: number;
    available_offers: number;
    priced_offers: number;
    available_priced_offers: number;
    average_price: number | null;
    retailer_count: number;
    price_bands: { from: number; to: number | null; count: number }[];
  } | null;
  matching_products: HomeProduct[] | null;
  catalog_filters: Record<string, string[]> | null;
  configuration_summary: {
    configuration_count: number;
    available_configurations: number;
    shared_configurations: number;
    average_spread_percent: number | null;
    average_spread_dt: number | null;
    price_bands: { from: number; to: number | null; count: number }[];
    spread_bands: { label: string; count: number }[];
  } | null;
  price_changes: {
    since: string | null;
    until: string;
    compared_offers: number;
    excluded_offers: number;
    fell: number;
    rose: number;
    unchanged: number;
    selected: 'fell' | 'rose' | 'unchanged';
    page: number;
    pages: number;
    offers: {
      id: string;
      title: string;
      retailer: string;
      before: number;
      after: number;
      change_percent: number;
      in_stock: boolean;
    }[];
    summary: {
      fell: PriceChangeDirectionSummary;
      rose: PriceChangeDirectionSummary;
    } | null;
    magnitude_bands: { from: number; to: number | null; fell: number; rose: number }[] | null;
  } | null;
  supported_sections: string[];
  generated_at: string;
  last_scan_at: string | null;
  retailer_overviews: Record<
    string,
    {
      comparable_configurations: number;
      retailers: {
        name: string;
        offers: number;
        in_stock_offers?: number;
        lowest_price_wins: number;
        price_comparisons: number;
        within_two_percent?: number;
        two_to_five_percent?: number;
        above_five_percent?: number;
      }[];
    }
  >;
  explorer_charts: StatisticChart[];
  facets: { key: string; label: string; options: { value: string; count: number }[] }[];
  retailers: string[];
  configuration_count: number;
  configuration_page: number;
  configuration_pages: number;
  category_trends: {
    section: string;
    change_percent: number | null;
    comparable_offers: number;
    since: string | null;
  }[];
  highlights: HomeProduct[];
  configurations: {
    id: string;
    section?: string;
    title: string;
    product_id: string | null;
    offers: number;
    retailer_count: number;
    current_price: number | null;
    in_stock: number;
    price_points: StatisticPoint[];
    availability_points: StatisticPoint[];
  }[];
}

export interface PriceChangeDirectionSummary {
  count: number;
  average_percent: number | null;
  average_amount_dt: number | null;
  largest_percent: number | null;
  largest_amount_dt: number | null;
}
