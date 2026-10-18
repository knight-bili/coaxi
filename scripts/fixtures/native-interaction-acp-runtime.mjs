#!/usr/bin/env node
// Test-owned ACP peer; never connects to a provider or executes Built-in CLI.
import { createInterface } from 'node:readline'
import { appendFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const sessionId = 'controlled-native-session'
const trace = fileURLToPath(new URL('wire.jsonl', import.meta.url))
const send = value => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', ...value })}\n`)
const question = id => send({ id, method: '_x.ai/ask_user_question', params: {
  sessionId, toolCallId: `private-${id}`,
  questions: [{ question: '先做哪一项？', options: [{ label: '接口' }, { label: '界面' }] }]
} })
const expected = new Set([' question rpc ', 'plan-rpc'])
const replyIds = new Set([...expected, 'unanswered-rpc'])
if (process.argv.includes('--version')) {
  process.stdout.write('1.0.0\n')
  process.exit(0)
}
for await (const line of createInterface({ input: process.stdin })) {
  const message = JSON.parse(line)
  if (message.method === 'initialize') {
    send({ id: message.id, result: { protocolVersion: 1, agentCapabilities: {},
      authMethods: [{ id: 'cached_token', name: 'Controlled test credential' }]
    } })
  } else if (message.method === 'authenticate') {
    send({ id: message.id, result: {} })
  } else if (message.method === 'session/new') {
    send({ id: message.id, result: { sessionId, configOptions: [], models: {
      currentModelId: 'fixture-model', availableModels: [{ modelId: 'fixture-model', name: 'Fixture' }]
    } } })
  } else if (message.method === 'session/prompt') {
    // Both must reach Core before any User answer. A reader blocked on the first
    // question cannot admit the second request.
    question(' question rpc ')
    send({ id: 'plan-rpc', method: '_x.ai/exit_plan_mode', params: {
      sessionId, planContent: '受控计划：兼容旧版本。'
    } })
  } else if (!message.method && message.id === 'fixture-exit') {
    process.exit(0)
  } else if (!message.method && replyIds.has(message.id)) {
    // Only test RPC replies are recorded; launch secrets and prompt bodies are not.
    appendFileSync(trace, `${JSON.stringify(message)}\n`)
    const firstReply = expected.delete(message.id)
    if (firstReply && expected.size === 0) question('unanswered-rpc')
  } else if (message.method && message.id !== undefined) {
    send({ id: message.id, error: { code: -32601, message: 'Controlled method unavailable' } })
  }
}
