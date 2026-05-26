import { html, nothing, type TemplateResult } from "lit";
import { unsafeSVG } from "lit/directives/unsafe-svg.js";
import type { IconNode, SVGProps } from "lucide";

export function renderIcon(node: IconNode | undefined): TemplateResult | typeof nothing {
  return node ? html`${unsafeSVG(toSvg(node))}` : nothing;
}

function toSvg(node: IconNode): string {
  const childText = node.map(([tag, attrs]) => `<${tag}${attrsToString(attrs)} />`).join("");
  return `<svg${attrsToString({
    xmlns: "http://www.w3.org/2000/svg",
    width: 24,
    height: 24,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    "stroke-width": 2,
    "stroke-linecap": "round",
    "stroke-linejoin": "round",
  })}>${childText}</svg>`;
}

function attrsToString(attrs: SVGProps): string {
  return Object.entries(attrs)
    .filter((entry): entry is [string, string | number] => entry[1] !== undefined)
    .map(([key, value]) => ` ${key}="${String(value).replace(/"/g, "&quot;")}"`)
    .join("");
}
