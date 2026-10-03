import { createModels, createProvider, type Model } from "@earendil-works/pi-ai";
import { openAICompletionsApi } from "@earendil-works/pi-ai/api/openai-completions.lazy";

export const PI_SDK_VERSION = "0.99.2";
export const GATEWAY_MODEL_IDS = [
  "space-bunny",
  "openrouter/space-bunny",
  "openrouter/apodex/apodex-1.1-mini:free",
  "opencode/space-bunny",
  "commandcode/space-bunny",
] as const;

export function gatewayPiSnippet(baseUrl: string, modelId: string) {
  return `// npm install @earendil-works/pi-ai@${PI_SDK_VERSION}
import { createModels, createProvider } from '@earendil-works/pi-ai';
import { openAICompletionsApi } from '@earendil-works/pi-ai/api/openai-completions.lazy';

const models = createModels();
models.setProvider(createProvider({
  id: 'free-router',
  auth: { apiKey: { name: 'Gateway key', resolve: async () => ({ auth: {} }) } },
  api: openAICompletionsApi(),
  models: [{
    id: ${JSON.stringify(modelId)}, name: 'Space Bunny',
    api: 'openai-completions', provider: 'free-router',
    baseUrl: ${JSON.stringify(baseUrl)},
    reasoning: true, input: ['text'],
    contextWindow: 32768, maxTokens: 4096,
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    compat: { supportsStore: false, supportsUsageInStreaming: false,
      supportsDeveloperRole: false, supportsReasoningEffort: true,
      maxTokensField: 'max_tokens' },
  }],
}));
const stream = models.stream(models.getModel('free-router', ${JSON.stringify(modelId)}), {
  messages: [{ role: 'user', content: '你好', timestamp: Date.now() }],
}, { apiKey: process.env.GATEWAY_API_KEY || 'local', maxTokens: 4096 });
for await (const event of stream) {
  if (event.type === 'text_delta') process.stdout.write(event.delta);
}
const result = await stream.result();
if (result.stopReason === 'error') throw new Error(result.errorMessage);`;
}

// Only the gateway credential enters the SDK. Upstream credentials stay in Rust.
export function createGatewayModels(baseUrl: string) {
  const models = createModels();
  const catalog: Model<"openai-completions">[] = GATEWAY_MODEL_IDS.map((id) => ({
    id,
    name: id,
    api: "openai-completions",
    provider: "free-router",
    baseUrl,
    reasoning: true,
    input: ["text"],
    cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
    // Conservative local defaults, not an assertion about upstream limits.
    contextWindow: 32768,
    maxTokens: 4096,
    compat: {
      supportsStore: false,
      supportsDeveloperRole: false,
      supportsReasoningEffort: true,
      supportsUsageInStreaming: false,
      maxTokensField: "max_tokens",
    },
  }));
  models.setProvider(createProvider({
    id: "free-router",
    name: "Free Router",
    baseUrl,
    auth: { apiKey: { name: "Gateway API Key", resolve: async () => ({ auth: {} }) } },
    models: catalog,
    api: openAICompletionsApi(),
  }));
  return models;
}

export async function streamGatewayReply(options: {
  baseUrl: string;
  modelId: string;
  apiKey: string;
  prompt: string;
  signal: AbortSignal;
  onText: (text: string) => void;
  onThinking: (text: string) => void;
}) {
  const models = createGatewayModels(options.baseUrl);
  const model = models.getModel("free-router", options.modelId);
  if (!model) throw new Error("未知网关模型");
  const stream = models.stream(model, {
    messages: [{ role: "user", content: options.prompt, timestamp: Date.now() }],
  }, { apiKey: options.apiKey, signal: options.signal, maxTokens: 4096 });
  let text = "", thinking = "";
  for await (const event of stream) {
    if (event.type === "text_delta") options.onText(text += event.delta);
    if (event.type === "thinking_delta") options.onThinking(thinking += event.delta);
  }
  const message = await stream.result();
  if (message.stopReason === "error") throw new Error(message.errorMessage || "模型调用失败");
  return message;
}
