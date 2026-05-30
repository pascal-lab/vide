import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, extname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const docsRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const distRoot = resolve(docsRoot, 'dist');
const site = new URL(process.env.ASTRO_SITE ?? 'https://vide.pascal-lab.net');
const basePath = normalizeBasePath(process.env.ASTRO_BASE ?? '/');
const ignoredProtocols = new Set(['mailto:', 'tel:', 'javascript:', 'data:', 'blob:']);

if (!existsSync(distRoot)) {
  console.error(`Docs dist directory not found: ${distRoot}`);
  process.exit(1);
}

const htmlFiles = listFiles(distRoot).filter((file) => extname(file) === '.html');
const failures = [];

for (const htmlFile of htmlFiles) {
  const html = readFileSync(htmlFile, 'utf8');
  const pagePath = toPagePath(htmlFile);

  for (const { attribute, value } of extractLinks(html)) {
    const target = value.trim();

    if (!target || target.startsWith('#') || target.startsWith('{{')) {
      continue;
    }

    let url;
    try {
      url = new URL(target, new URL(pagePath, site));
    } catch {
      failures.push(`${formatPath(htmlFile)}: invalid ${attribute}="${target}"`);
      continue;
    }

    if (ignoredProtocols.has(url.protocol)) {
      continue;
    }

    if (url.origin !== site.origin) {
      continue;
    }

    const localPath = stripBasePath(url.pathname);
    const resolved = resolveLocalPath(localPath);

    if (!resolved) {
      failures.push(`${formatPath(htmlFile)}: missing ${attribute}="${target}" -> ${url.pathname}`);
      continue;
    }

    if (url.hash && extname(resolved) === '.html' && !hasAnchor(resolved, decodeURIComponent(url.hash.slice(1)))) {
      failures.push(`${formatPath(htmlFile)}: missing anchor ${attribute}="${target}" -> ${url.pathname}${url.hash}`);
    }
  }
}

if (failures.length > 0) {
  console.error(`Found ${failures.length} broken docs link${failures.length === 1 ? '' : 's'}:`);
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(`Checked links in ${htmlFiles.length} generated docs page${htmlFiles.length === 1 ? '' : 's'}.`);

function listFiles(directory) {
  const entries = readdirSync(directory, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const entryPath = join(directory, entry.name);
    return entry.isDirectory() ? listFiles(entryPath) : [entryPath];
  });
}

function extractLinks(html) {
  const links = [];
  const pattern = /\b(href|src)\s*=\s*(["'])(.*?)\2/gi;
  let match;

  while ((match = pattern.exec(html)) !== null) {
    links.push({ attribute: match[1].toLowerCase(), value: decodeHtml(match[3]) });
  }

  return links;
}

function resolveLocalPath(pathname) {
  const normalizedPath = pathname.replace(/^\/+/, '');
  const candidates = [];

  if (pathname.endsWith('/')) {
    candidates.push(join(distRoot, normalizedPath, 'index.html'));
  } else {
    candidates.push(join(distRoot, normalizedPath));
    candidates.push(join(distRoot, normalizedPath, 'index.html'));
    candidates.push(join(distRoot, `${normalizedPath}.html`));
  }

  return candidates.find((candidate) => existsSync(candidate) && statSync(candidate).isFile());
}

function hasAnchor(htmlFile, anchor) {
  if (!anchor) {
    return true;
  }

  const html = readFileSync(htmlFile, 'utf8');
  const escapedAnchor = escapeRegExp(anchor);
  return new RegExp(`\\b(?:id|name)=["']${escapedAnchor}["']`, 'i').test(html);
}

function toPagePath(htmlFile) {
  const path = relative(distRoot, htmlFile).split(sep).join('/');

  if (path === 'index.html') {
    return basePath;
  }

  return `${basePath}${path.replace(/(?:^|\/)index\.html$/, '/')}`;
}

function stripBasePath(pathname) {
  if (basePath === '/') {
    return pathname;
  }

  return pathname.startsWith(basePath) ? `/${pathname.slice(basePath.length)}` : pathname;
}

function normalizeBasePath(pathname) {
  const withSlashes = `/${pathname}/`.replace(/\/+/g, '/');
  return withSlashes === '//' ? '/' : withSlashes;
}

function decodeHtml(value) {
  return value
    .replace(/&amp;/g, '&')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>');
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function formatPath(path) {
  return relative(docsRoot, path).split(sep).join('/');
}
