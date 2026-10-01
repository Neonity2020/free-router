import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { transformWithEsbuild } from 'vite';

test('Markdown renders GFM and incomplete streams without executing model HTML', async (t) => {
  const frontend = resolve(import.meta.dirname, '..');
  const directory = await mkdtemp(resolve(frontend, '.markdown-test-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const source = resolve(frontend, 'src/MarkdownMessage.tsx');
  const compiled = await transformWithEsbuild(await readFile(source, 'utf8'), source, {
    loader: 'tsx', jsx: 'automatic', format: 'esm',
  });
  const modulePath = resolve(directory, 'MarkdownMessage.mjs');
  await writeFile(modulePath, compiled.code);
  const { default: MarkdownMessage } = await import(pathToFileURL(modulePath).href);
  const render = text => renderToStaticMarkup(createElement(MarkdownMessage, { text }));
  const markdown = '# 编程结果\n\n**完成**，使用 `node`。\n\n- [x] 测试通过\n- [ ] 发布\n\n> 引用说明\n\n| 模型 | 状态 |\n| --- | --- |\n| Space Bunny | 正常 |\n\n```js\nconsole.log("你好");\n```\n\n[文档](https://example.com)';
  const html = render(markdown);
  for (const tag of ['<h1>', '<strong>', '<blockquote>', '<table>', '<code class="language-js">']) assert.ok(html.includes(tag), tag);
  assert.match(html, /type="checkbox"/);
  assert.match(html, /aria-label="复制代码块"/);
  assert.match(html, /target="_blank" rel="noopener noreferrer"/);
  assert.match(html, /console\.log\(&quot;你好&quot;\)/);
  const unsafe = render('<script>alert(1)</script>\n\n<img src=x onerror=alert(2)>\n\n[危险](javascript:alert%281%29)');
  assert.doesNotMatch(unsafe, /<script|onerror=|href="javascript:/);
  assert.match(render('```js\nconst partial ='), /const partial =/);
});
