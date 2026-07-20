import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { parseArgs } from 'node:util';

import matter from 'gray-matter';
import { toString } from 'mdast-util-to-string';
import remarkGfm from 'remark-gfm';
import remarkMdx from 'remark-mdx';
import remarkParse from 'remark-parse';
import remarkStringify from 'remark-stringify';
import { unified } from 'unified';
import { visit } from 'unist-util-visit';

const repo = 'pascal-lab/vide';
const docsSite = 'https://vide.pascal-lab.net';
const docsRoot = 'docs/src/content/docs';

const { values } = parseArgs({
  options: {
    tag: { type: 'string' },
    output: { type: 'string' },
  },
});

if (!values.tag || !values.output) {
  console.error(
    'Usage: node docs/scripts/release-body-from-changelog.mjs --tag <vX.Y.Z> --output <path>'
  );
  process.exit(2);
}

const tag = values.tag;
const output = values.output;
const page = tag.replaceAll('.', '-');
const sourceDir = `en/changelog/${page}`;
const source = path.posix.join(docsRoot, sourceDir, 'index.mdx');

if (!fs.existsSync(source)) {
  console.error(`Missing ${source}`);
  console.error(`Add an English changelog page for ${tag} before publishing the release.`);
  process.exit(1);
}

const sourceText = fs.readFileSync(source, 'utf8');
const { content } = matter(sourceText);
const parser = unified().use(remarkParse).use(remarkMdx).use(remarkGfm);
const tree = parser.parse(content);

tree.children = tree.children.filter((node) => node.type !== 'mdxjsEsm');

visit(tree, ['link', 'image', 'definition'], (node) => {
  if (!node.url || isExternalTarget(node.url)) {
    return;
  }

  node.url =
    node.type === 'image'
      ? assetUrl(sourceDir, tag, node.url)
      : docsUrl(sourceDir, node.url);
});

if (!toString(tree).trim()) {
  console.error(`Changelog page for ${tag} has no body content: ${source}`);
  process.exit(1);
}

const body = unified()
  .use(remarkStringify, {
    bullet: '-',
    emphasis: '*',
    fences: true,
    listItemIndent: 'one',
    rule: '-',
    strong: '*',
  })
  .stringify(tree)
  .trimEnd();

fs.writeFileSync(output, `${body}\n`);

function isExternalTarget(target) {
  return (
    target.startsWith('#') ||
    /^[a-z][a-z0-9+.-]*:/i.test(target) ||
    target.startsWith('//')
  );
}

function assetUrl(sourceRouteDir, tagName, target) {
  return new URL(
    target,
    `https://raw.githubusercontent.com/${repo}/${tagName}/${docsRoot}/${sourceRouteDir}/`
  ).toString();
}

function docsUrl(sourceRouteDir, target) {
  const url = new URL(target, `${docsSite}/${sourceRouteDir}/`);
  if (url.pathname.endsWith('/index')) {
    url.pathname = url.pathname.slice(0, -'/index'.length);
  }
  if (!url.pathname.endsWith('/')) {
    url.pathname += '/';
  }
  return url.toString();
}
