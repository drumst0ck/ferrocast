export const SITE = 'https://ferrocast.app';

export function breadcrumbs(items: [string, string][]) {
  return {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: items.map(([name, path], i) => ({ '@type': 'ListItem', position: i + 1, name, item: `${SITE}${path}` })),
  };
}

export function howTo(name: string, description: string, steps: [string, string][]) {
  return {
    '@context': 'https://schema.org',
    '@type': 'HowTo',
    name,
    description,
    step: steps.map(([stepName, text], i) => ({ '@type': 'HowToStep', position: i + 1, name: stepName, text })),
  };
}
