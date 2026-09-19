import { useEffect, useRef, useState } from 'react';
import type { CatalogQuery, PageResponse, SectionInfo, ServerInfo } from '../types';
import { parsePage, parseServerInfo, request } from '../lib/api';
import { defaultQuery, apiQueryParams, sectionInfoKey, queryURL, readQuery } from '../lib/catalog';

export function useCatalog(shouldFetchCatalogPage = true) {
  const [query, setQuery] = useState(() => readQuery(window.location));
  const [categories, setCategories] = useState<ServerInfo['categories']>({});
  const [catalogPage, setCatalogPage] = useState<PageResponse | null>(null);
  const [baselineSectionInfo, setBaselineSectionInfo] = useState<SectionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [reloadNonce, setReloadNonce] = useState(0);
  const sectionInfoCache = useRef(new Map<string, SectionInfo>());
  const lastSectionInfoKey = useRef<string | null>(null);
  const serverInfoCache = useRef<ServerInfo | null>(null);

  useEffect(() => {
    const pop = () => setQuery(readQuery(window.location));
    window.addEventListener('popstate', pop);
    return () => window.removeEventListener('popstate', pop);
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError('');
    setCatalogPage(null);
    setBaselineSectionInfo(sectionInfoCache.current.get(query.section) || null);
    const load = async () => {
      try {
        const info =
          serverInfoCache.current || parseServerInfo(await request('/info', controller.signal));
        if (controller.signal.aborted) return;
        serverInfoCache.current = info;
        setCategories(info.categories);
        if (!shouldFetchCatalogPage) return;
        const available = Object.values(info.categories).flat();
        if (!available.length) return;
        const category = available.find(
          (category) => category.id.toLowerCase() === query.section.toLowerCase(),
        );
        if (!category) throw new Error('This category is not available. Choose another category.');
        const result = parsePage(
          await request(
            `${`/${category.id}/products`}?${apiQueryParams(
              query,
              lastSectionInfoKey.current !== sectionInfoKey(query),
            )}`,
            controller.signal,
          ),
        );
        if (controller.signal.aborted) return;
        if (result.section_info) {
          sectionInfoCache.current.set(query.section, result.section_info);
          lastSectionInfoKey.current = sectionInfoKey(query);
        }
        setBaselineSectionInfo(
          result.section_info || sectionInfoCache.current.get(query.section) || null,
        );
        if (query.page !== result.page) {
          const next = { ...query, page: result.page };
          window.history.replaceState(null, '', queryURL(next));
          setQuery(next);
          return;
        }
        setCatalogPage(result);
      } catch (cause) {
        if (!controller.signal.aborted)
          setError(cause instanceof Error ? cause.message : 'The catalog could not be loaded.');
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    };
    void load();
    return () => controller.abort();
  }, [shouldFetchCatalogPage, query, reloadNonce]);

  const update = (changes: Partial<CatalogQuery>, replace = false) => {
    const next = { ...query, ...changes, page: changes.page ?? 1 };
    window.history[replace ? 'replaceState' : 'pushState'](null, '', queryURL(next));
    setQuery(next);
  };
  return {
    query,
    categories,
    catalogPage,
    baselineSectionInfo,
    loading,
    error,
    update,
    retry: () => setReloadNonce((value) => value + 1),
    changeSection: (url: string) =>
      update(readQuery(new URL(url.replace(/^\/?/, '/'), window.location.origin))),
    reset: () =>
      update({
        ...defaultQuery(query.section),
        sort: query.sort,
        grouped: query.grouped,
        groupBy: query.groupBy,
      }),
  };
}
