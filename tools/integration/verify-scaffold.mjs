import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (rel) => JSON.parse(readFileSync(path.join(root, rel), 'utf8'));
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', maxBuffer: 20 * 1024 * 1024 });
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const source = read('integration/sources.lock.json');
const features = read('integration/planning/features.json');
const cases = read('integration/acceptance/cases.json');
const tasks = read('integration/planning/work-packages.json');
const schedule = read('integration/planning/schedule.json');
const runtime = read('integration/planning/runtime-matrix.json');
const entries = read('integration/inventory/source-entrypoints.json');
const ids = new Set(features.map((x) => x.id));
const caseIds = new Set(cases.map((x) => x.id));
const phases = new Set(schedule.phases.map((x) => x.id));
assert(source.sources.length === 4, '必须锁定四份源码');
assert(ids.size === features.length && features.length >= 81, '来源能力缺失或重复');
assert(caseIds.size === cases.length && cases.length >= 324, '场景缺失或重复');
assert(new Set(tasks.map((x) => x.id)).size === tasks.length && tasks.length >= 48, '工作包缺失或重复');
for (const s of source.sources) {
  assert(/^[a-f0-9]{40}$/.test(s.commit), `非法来源提交 ${s.id}`);
  for (const license of s.retained_licenses) {
    assert(!path.isAbsolute(license.retained_path) && !license.retained_path.includes('..'), '许可证路径越界');
    const data = readFileSync(path.join(root, license.retained_path));
    assert(createHash('sha256').update(data).digest('hex') === license.sha256, `许可证被修改 ${s.id}`);
  }
}
const base = source.sources.find((x) => x.id === 'rovai-ai').commit;
git('merge-base', '--is-ancestor', base, 'HEAD');
for (const f of features) {
  assert(f.scope === '一期必须' && phases.has(f.phase), `来源范围或阶段错误 ${f.id}`);
  assert(source.sources.some((s) => s.id === f.repo && f.url.includes(s.commit)), `来源链接未固定 ${f.id}`);
  assert(/^[a-f0-9]{64}$/.test(f.source_sha256), `来源hash缺失 ${f.id}`);
  assert(f.primary_work_packages.length > 0 && f.primary_work_packages.every((id) => tasks.some((t) => t.id === id && t.phase === f.phase)), `工作包映射缺失 ${f.id}`);
  assert(f.acceptance_cases.length >= 4 && f.acceptance_cases.every((id) => cases.some((c) => c.id === id && c.feature === f.id && c.phase === f.phase)), `场景映射缺失 ${f.id}`);
}
for (const c of cases) {
  assert(ids.has(c.feature) && phases.has(c.phase) && c.steps && c.expected && c.platforms.length, `场景不完整 ${c.id}`);
  assert(['not_run', 'passed', 'failed', 'blocked'].includes(c.status), `场景状态错误 ${c.id}`);
  assert(c.status !== 'passed' || c.evidence, `通过用例缺少证据 ${c.id}`);
}
assert(runtime.runtimes.length >= 21 && runtime.custom_acp, 'Runtime并集缺失');
assert(entries.count === entries.entries.length && entries.count > 0, '入口库存不完整');
const weekdayCount = (start, end) => {
  const date = new Date(`${start}T00:00:00Z`), last = new Date(`${end}T00:00:00Z`); let count = 0;
  assert(Number.isFinite(date.valueOf()) && Number.isFinite(last.valueOf()) && date <= last, '非法日期');
  while (date <= last) { if (date.getUTCDay() !== 0 && date.getUTCDay() !== 6) count++; date.setUTCDate(date.getUTCDate() + 1); }
  return count;
};
for (const p of schedule.phases) assert(weekdayCount(p.start, p.end) === p.days, `阶段日期天数不一致 ${p.id}`);
assert(schedule.phases.reduce((sum, p) => sum + p.days, 0) === schedule.total_days, '总天数不一致');
assert(schedule.base_days + schedule.reserve_days === schedule.total_days, '预留天数不一致');

// 只扫描融合新增或改动文件，源历史不被误报为已经做过全历史审计。
const changed = new Set([
  ...git('diff', '--name-only', base, '--').trim().split(/\r?\n/),
  ...git('ls-files', '--others', '--exclude-standard').trim().split(/\r?\n/),
]);
const sensitivePath = /(^|\/)(\.env(?:\..+)?|id_rsa|id_ed25519)$|\.(?:p12|jks|keystore|mobileprovision)$/i;
const sensitiveContent = [/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/, /\bgh[pousr]_[A-Za-z0-9]{30,}\b/, /\bgithub_pat_[A-Za-z0-9_]{40,}\b/, /\bAKIA[A-Z0-9]{16}\b/];
for (const file of changed) {
  if (!file || !existsSync(path.join(root, file))) continue;
  assert(!sensitivePath.test(file) || file.endsWith('.env.example'), `新增敏感文件 ${file}`);
  if (/\.(?:json|md|csv|mjs|yml|yaml|txt)$/.test(file)) {
    const content = readFileSync(path.join(root, file), 'utf8');
    assert(!sensitiveContent.some((pattern) => pattern.test(content)), `新增文件疑似包含秘密 ${file}`);
  }
}
console.log(JSON.stringify({status:'passed', features:features.length, cases:cases.length, workPackages:tasks.length, sourceEntrypoints:entries.count, runtimeKinds:runtime.runtimes.length, totalDays:schedule.total_days, productExecution:'not_run'}));
