import { useCallback, useEffect, useRef, useState } from 'react';
import type { CatalogQuery, PageResponse, SectionInfo, ServerInfo } from '../types';
import { parsePage, parseServerInfo, request } from '../lib/api';
import {
  configureCatalogDefaults,
  defaultQuery,
  apiQueryParams,
  sectionInfoKey,
  queryURL,
  readQuery,
} from '../lib/catalog';

const emptyCategories: ServerInfo['categories'] = {};

export function useCatalog(
  shouldFetchCatalogPage = true,
  background = window.location.pathname + window.location.search,
) {
  const [query, setQuery] = useState(() => readQuery(new URL(background, window.location.origin)));
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [catalogPage, setCatalogPage] = useState<PageResponse | null>(null);
  const [baselineSectionInfo, setBaselineSectionInfo] = useState<SectionInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [reloadNonce, setReloadNonce] = useState(0);
  const sectionInfoCache = useRef(new Map<string, SectionInfo>());
  const lastSectionInfoKey = useRef<string | null>(null);
  const serverInfoCache = useRef<ServerInfo | null>(null);
  const currentBackground = useRef(background);
  currentBackground.current = background;

  useEffect(() => {
    const next = readQuery(new URL(background, window.location.origin));
    setQuery((current) => (JSON.stringify(current) === JSON.stringify(next) ? current : next));
  }, [background]);

  useEffect(() => {
    if (serverInfoCache.current) {
      setServerInfo(serverInfoCache.current);
      return;
    }
    const controller = new AbortController();
    const load = async () => {
      try {
        const info = parseServerInfo(await request('/info', controller.signal));
        if (controller.signal.aborted) return;
        serverInfoCache.current = info;
        configureCatalogDefaults(info.defaults);
        setQuery(readQuery(new URL(currentBackground.current, window.location.origin)));
        setServerInfo(info);
      } catch (cause) {
        if (!controller.signal.aborted) {
          setError(cause instanceof Error ? cause.message : 'The catalog could not be loaded.');
          setLoading(false);
        }
      }
    };
    void load();
    return () => controller.abort();
  }, [reloadNonce]);

  useEffect(() => {
    if (!shouldFetchCatalogPage) {
      setLoading(false);
      setCatalogPage(null);
      return;
    }
    if (!serverInfo) {
      setLoading(true);
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError('');
    setCatalogPage(null);
    setBaselineSectionInfo(sectionInfoCache.current.get(query.section) || null);
    const load = async () => {
      try {
        const available = Object.values(serverInfo.categories).flat();
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
  }, [shouldFetchCatalogPage, query, reloadNonce, serverInfo]);

  const update = useCallback((changes: Partial<CatalogQuery>, replace = false) => {
    setQuery((current) => {
      const next = { ...current, ...changes, page: changes.page ?? 1 };
      window.history[replace ? 'replaceState' : 'pushState'](null, '', queryURL(next));
      return next;
    });
  }, []);
  const changeSection = useCallback(
    (url: string) => update(readQuery(new URL(url.replace(/^\/?/, '/'), window.location.origin))),
    [update],
  );
  const reset = useCallback(() => {
    setQuery((current) => {
      const next = {
        ...defaultQuery(current.section),
        sort: current.sort,
        grouped: current.grouped,
        groupBy: current.groupBy,
      };
      window.history.pushState(null, '', queryURL(next));
      return next;
    });
  }, []);
  return {
    query,
    categories: serverInfo?.categories || emptyCategories,
    catalogPage,
    baselineSectionInfo,
    loading,
    error,
    update,
    retry: () => setReloadNonce((value) => value + 1),
    changeSection,
    reset,
  };
}
