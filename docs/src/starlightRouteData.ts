import type { StarlightRouteData } from '@astrojs/starlight/route-data';

type SidebarEntry = StarlightRouteData['sidebar'][number];
type SidebarLink = Extract<SidebarEntry, { type: 'link' }>;

const sectionLabels = {
  'user-guide': new Set(['用户手册', 'User Guide']),
  'advanced-guide': new Set(['进阶', 'Advanced']),
  changelog: new Set(['Changelog']),
  playground: new Set(['Playground']),
} as const;

type Section = keyof typeof sectionLabels;

export function onRequest(
  context: { locals: { starlightRoute: StarlightRouteData } },
  next: () => Promise<void>
) {
  const route = context.locals.starlightRoute;
  const routeId =
    route.locale && route.id.startsWith(`${route.locale}/`)
      ? route.id.slice(route.locale.length + 1)
      : route.id;
  const section = getSection(routeId);

  if (!section) {
    return next();
  }

  const sectionSidebar = route.sidebar.filter((entry) => {
    return entry.type === 'group' && sectionLabels[section].has(entry.label);
  });
  route.pagination = getPagination(sectionSidebar);

  return next();
}

function getSection(routeId: string): Section | undefined {
  if (routeId.startsWith('user-guide')) return 'user-guide';
  if (routeId.startsWith('advanced-guide')) return 'advanced-guide';
  if (routeId.startsWith('changelog')) return 'changelog';
  if (routeId === 'playground') return 'playground';
}

function getPagination(sidebar: SidebarEntry[]): StarlightRouteData['pagination'] {
  const links = flatLinks(sidebar);
  const currentIndex = links.findIndex((link) => link.isCurrent);
  if (currentIndex < 0) {
    return { prev: undefined, next: undefined };
  }

  return {
    prev: links[currentIndex - 1],
    next: links[currentIndex + 1],
  };
}

function flatLinks(entries: SidebarEntry[]): SidebarLink[] {
  return entries.flatMap((entry) => {
    if (entry.type === 'link') return [entry];
    return flatLinks(entry.entries);
  });
}
