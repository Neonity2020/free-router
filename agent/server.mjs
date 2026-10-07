import { createServer } from 'node:http';
import { recordToolEvent } from './tool-events.mjs';
import { createWebSearchTool } from './web-search.mjs';
import { randomUUID } from 'node:crypto';
import { realpath, stat } from 'node:fs/promises';
import { isAbsolute } from 'node:path';
import { createProvider, envApiKeyAuth, InMemoryCredentialStore, InMemoryModelsStore } from '@earendil-works/pi-ai';
import { openAICompletionsApi } from '@earendil-works/pi-ai/api/openai-completions.lazy';
import { createAgentSession, ModelRuntime, DefaultResourceLoader, SessionManager, SettingsManager } from '@earendil-works/pi-coding-agent';

const token = process.env.PI_BRIDGE_TOKEN;
const baseUrl = process.env.PI_GATEWAY_URL;
const defaultCwd = process.env.PI_DEFAULT_CWD || process.cwd();
delete process.env.PI_BRIDGE_TOKEN;
if (!token || !baseUrl) throw new Error('Agent must be launched by Free Router');
const models = ['space-bunny', 'openrouter/free', 'opencode/space-bunny', 'commandcode/space-bunny'];
const sessions = new Map();
const limitText = (value, limit = 24000) => {
  const text = typeof value === 'string' ? value : JSON.stringify(value ?? '', null, 2);
  return text.length > limit ? `${text.slice(0, limit)}\n…输出已截断` : text;
};
const toolText = result => result?.content?.filter(c => c.type === 'text').map(c => c.text).join('\n') || '';
function addItem(state, item) {
  const entry = { id: randomUUID(), ...item };
  state.items.push(entry);
  if (state.items.length > 160) state.items.shift();
  return entry;
}
function snapshot(state) {
  return { id: state.id, cwd: state.cwd, model: state.model, busy: state.busy,
    title: state.title, tools: state.session.getActiveToolNames(), items: state.items };
}
function observe(state, event) {
  if (event.type === 'message_start' && event.message.role === 'assistant') {
    state.assistant = addItem(state, { type: 'assistant', text: '', thinking: '' });
  }
  if (event.type === 'message_update') {
    const update = event.assistantMessageEvent;
    if (!state.assistant) return;
    if (update.type === 'text_delta') state.assistant.text = limitText(state.assistant.text + update.delta, 80000);
    if (update.type === 'thinking_delta') state.assistant.thinking = limitText(state.assistant.thinking + update.delta);
  }
  if (event.type === 'message_end' && event.message.role === 'assistant' && state.assistant) {
    state.assistant.text = limitText(toolText(event.message), 80000);
    state.assistant.thinking = limitText(event.message.content.filter(c => c.type === 'thinking').map(c => c.thinking).join('\n'));
    if (event.message.stopReason === 'error') addItem(state, { type: 'notice', error: true, text: event.message.errorMessage || '模型调用失败' });
  }
  recordToolEvent(state, event, addItem, limitText, toolText);
}
async function createSession(body, key, exaKey) {
  if (sessions.size >= 12) throw Object.assign(new Error('最多保留 12 个会话，请先删除旧会话'), { code: 409 });
  if (!models.includes(body.model)) throw new Error('未知网关模型');
  if (typeof body.cwd !== 'string' || !isAbsolute(body.cwd)) throw new Error('请输入绝对工作目录');
  const cwd = await realpath(body.cwd);
  if (!(await stat(cwd)).isDirectory()) throw new Error('工作目录必须是已存在的文件夹');
  const runtime = await ModelRuntime.create({
    credentials: new InMemoryCredentialStore(), modelsStore: new InMemoryModelsStore(),
    modelsPath: null, refreshOnCreate: false,
  });
  runtime.registerNativeProvider(createProvider({
    id: 'free-router', name: 'Free Router', baseUrl,
    auth: { apiKey: envApiKeyAuth('Gateway key', []) }, api: openAICompletionsApi(),
    models: models.map(id => ({ id, name: id, provider: 'free-router', api: 'openai-completions',
      baseUrl, reasoning: true, input: ['text'], contextWindow: 32768, maxTokens: 4096,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      compat: { supportsStore: false, supportsDeveloperRole: false, supportsReasoningEffort: true,
        supportsUsageInStreaming: false, maxTokensField: 'max_tokens' },
    })),
  }));
  await runtime.setRuntimeApiKey('free-router', key);
  const settingsManager = SettingsManager.inMemory({
    retry: { enabled: false }, compaction: { enabled: true },
  });
  // Keep SDK's coding prompt and project context; never load user/global extensions.
  const resourceLoader = new DefaultResourceLoader({ cwd, agentDir: cwd,
    settingsManager, noExtensions: true, noSkills: true, noPromptTemplates: true, noThemes: true,
    appendSystemPrompt: ['你是 Free Router 中的 Pi 编程助手。使用工具完成用户的编程任务，并以用户的语言回答。工作目录不是安全沙箱；只操作用户任务需要的文件。'],
  });
  await resourceLoader.reload();
  const searchConfig = { key: exaKey || '', baseUrl: process.env.PI_EXA_BASE_URL || 'https://api.exa.ai' };
  const { session } = await createAgentSession({ cwd, agentDir: cwd, modelRuntime: runtime,
    model: runtime.getModel('free-router', body.model), thinkingLevel: 'high', resourceLoader,
    tools: ['read', 'write', 'edit', 'bash', 'grep', 'find', 'ls', 'web_search'], settingsManager,
    customTools: [createWebSearchTool(() => searchConfig)],
    sessionManager: SessionManager.inMemory(cwd),
  });
  const state = { id: randomUUID(), cwd, model: body.model, busy: false, title: '新会话',
    items: [], toolItems: new Map(), assistant: null, session, runtime, searchConfig };
  session.subscribe(event => observe(state, event));
  sessions.set(state.id, state);
  return snapshot(state);
}
async function bodyOf(req) {
  let body = '', length = 0;
  for await (const chunk of req) {
    length += chunk.length;
    if (length > 200000) throw new Error('请求过大');
    body += chunk;
  }
  return body ? JSON.parse(body) : {};
}
const server = createServer(async (req, res) => {
  res.setHeader('Content-Type', 'application/json');
  res.setHeader('Cache-Control', 'no-store');
  const respond = (code, data) => { res.writeHead(code); res.end(JSON.stringify(data)); };
  if (req.headers.authorization !== `Bearer ${token}`) return respond(401, { error: { message: 'Unauthorized' } });
  try {
    const path = new URL(req.url, 'http://localhost').pathname;
    const key = req.headers['x-pi-gateway-key'];
    if (req.method === 'GET' && path === '/status') return respond(200, { version: '0.99.2', default_cwd: defaultCwd, models });
    if (req.method === 'GET' && path === '/sessions') return respond(200, { sessions: [...sessions.values()].map(s => ({ id: s.id, cwd: s.cwd, model: s.model, busy: s.busy, title: s.title })) });
    if (req.method === 'POST' && path === '/sessions') return respond(201, await createSession(await bodyOf(req), key, req.headers['x-pi-exa-key']));
    const match = /^\/sessions\/([\w-]+)(?:\/(prompt|abort))?$/.exec(path);
    const state = match && sessions.get(match[1]);
    if (!state) return respond(404, { error: { message: '会话不存在，请新建会话' } });
    if (req.method === 'GET' && !match[2]) return respond(200, snapshot(state));
    if (req.method === 'POST' && match[2] === 'abort') {
      await state.session.abort();
      addItem(state, { type: 'notice', text: '任务已停止。已完成的文件修改保留。' });
      return respond(200, snapshot(state));
    }
    if (req.method === 'DELETE' && !match[2]) {
      if (state.busy) return respond(409, { error: { message: '请先停止正在运行的会话' } });
      state.session.dispose(); sessions.delete(state.id);
      return respond(200, { deleted: true });
    }
    if (req.method === 'POST' && match[2] === 'prompt') {
      const { prompt } = await bodyOf(req);
      if (typeof prompt !== 'string' || !prompt.trim() || prompt.length > 100000) throw new Error('请输入消息（最多 100000 字符）');
      if ([...sessions.values()].some(s => s.busy)) return respond(409, { error: { message: '另一个任务正在运行，请等待或停止' } });
      state.busy = true;
      state.searchConfig.key = req.headers['x-pi-exa-key'] || '';
      try { await state.runtime.setRuntimeApiKey('free-router', key); }
      catch (error) { state.busy = false; throw error; }
      if (state.title === '新会话') state.title = prompt.slice(0, 36);
      addItem(state, { type: 'user', text: prompt });
      // literal mode treats browser input as messages, not Pi slash commands/templates.
      state.session.prompt(prompt, { expandPromptTemplates: false }).catch(error => {
        addItem(state, { type: 'notice', error: true, text: limitText(error.message) });
      }).finally(() => {
        state.busy = false;
        for (const item of state.toolItems.values()) item.status = 'stopped';
        state.toolItems.clear();
      });
      return respond(202, snapshot(state));
    }
    respond(405, { error: { message: 'Method not allowed' } });
  } catch (error) {
    respond(error.code === 409 ? 409 : 400, { error: { message: error.message } });
  }
});
server.listen(0, '127.0.0.1', () => console.log(JSON.stringify({ port: server.address().port })));
// The Rust owner closes stdin on termination; stop tools and release the child.
process.stdin.resume();
process.stdin.on('end', shutdown);
process.on('SIGTERM', shutdown);
async function shutdown() {
  await Promise.allSettled([...sessions.values()].map(s => s.session.abort()));
  for (const state of sessions.values()) state.session.dispose();
  server.closeAllConnections(); server.close();
  process.exit(0);
}
