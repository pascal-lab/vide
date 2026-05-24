import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

type GrammarRule = {
  begin?: string;
  end?: string;
  include?: string;
  match?: string;
  name?: string;
  patterns?: GrammarRule[];
};

type Grammar = GrammarRule & {
  repository: Record<string, GrammarRule>;
};

function readGrammar(fileName: string): Grammar {
  return JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'syntaxes', fileName), 'utf8')) as Grammar;
}

function requireRule(rule: GrammarRule | undefined, message: string): GrammarRule {
  assert.ok(rule, message);
  return rule;
}

function patternIncludes(rule: GrammarRule): string[] {
  return rule.patterns?.map((pattern) => pattern.include).filter((include) => include !== undefined) ?? [];
}

function assertIncludesBefore(rule: GrammarRule, before: string, after: string, message: string): void {
  const includes = patternIncludes(rule);
  const beforeIndex = includes.indexOf(before);
  const afterIndex = includes.indexOf(after);

  assert.notEqual(beforeIndex, -1, `${message}: missing ${before}`);
  assert.notEqual(afterIndex, -1, `${message}: missing ${after}`);
  assert.ok(beforeIndex < afterIndex, `${message}: expected ${before} before ${after}`);
}

function collectDeclarationNames(grammar: Grammar, text: string): string[] {
  const rule = requireRule(grammar.repository.declaration_names.patterns?.[0], 'declaration name rule exists');
  assert.ok(rule.match, 'declaration name rule has a match pattern');

  return [...text.matchAll(new RegExp(rule.match, 'gm'))].map((match) => match[1]);
}

test('verilog grammar applies declaration rules before generic keyword rules', () => {
  const grammar = readGrammar('verilog.tmLanguage.json');
  assertIncludesBefore(grammar, '#declarations', '#keywords', 'top-level grammar');

  const moduleRule = requireRule(
    grammar.repository.module_pattern.patterns?.find((rule) => rule.name === 'meta.block.module.verilog'),
    'module grammar rule exists',
  );
  assertIncludesBefore(moduleRule, '#declarations', '#keywords', 'module grammar');
});

test('verilog declaration rules are not nested into instantiation bodies', () => {
  const grammar = readGrammar('verilog.tmLanguage.json');
  const instantiationRule = requireRule(
    grammar.repository.instantiation_patterns.patterns?.find(
      (rule) => rule.name === 'meta.block.instantiation.parameterless.verilog',
    ),
    'parameterless instantiation grammar rule exists',
  );

  assert.equal(patternIncludes(instantiationRule).includes('#declarations'), false);
});

test('verilog declaration names cover comma-separated and split-line declarations', () => {
  const grammar = readGrammar('verilog.tmLanguage.json');

  assert.deepEqual(
    collectDeclarationNames(
      grammar,
      `
        [15:0] x,
        y,
        zx, nx,
        [15:0] y1,
        y2,
        noty1;
      `,
    ),
    ['x', 'y', 'zx', 'nx', 'y1', 'y2', 'noty1'],
  );
  assert.deepEqual(collectDeclarationNames(grammar, '[WIDTH-1:0] data, other;'), ['data', 'other']);
});

test('verilog declaration blocks cover standard port, net, and reg declaration starts', () => {
  const grammar = readGrammar('verilog.tmLanguage.json');
  const declarations = grammar.repository.declarations.patterns ?? [];
  const portRule = requireRule(
    declarations.find((rule) => rule.name === 'meta.declaration.port.verilog'),
    'port declaration rule exists',
  );
  const variableRule = requireRule(
    declarations.find((rule) => rule.name === 'meta.declaration.variable.verilog'),
    'variable declaration rule exists',
  );
  const netRule = requireRule(
    declarations.find((rule) => rule.name === 'meta.declaration.net.verilog'),
    'net declaration rule exists',
  );

  assert.match('input [15:0] x,', new RegExp(portRule.begin ?? ''));
  assert.match('output zr, ng', new RegExp(portRule.begin ?? ''));
  assert.match('reg [15:0] r1, r2;', new RegExp(variableRule.begin ?? ''));
  assert.match('wire [15:0] x1, x2, notx1;', new RegExp(netRule.begin ?? ''));
  assert.match(' output next_port', new RegExp(portRule.end ?? ''));
  assert.match(');', new RegExp(portRule.end ?? ''));
});
